/*
 * getifaddrs()/freeifaddrs() for Android: bionic only exports them from API 24,
 * while the jniLibs are built against the API 21 sysroot (flutter/ndk_*.sh) and
 * webrtc-util calls them whenever WebRTC gathers ICE candidates.
 *
 * Only AF_INET and AF_INET6 entries are reported; the AF_PACKET ones the real
 * getifaddrs() also returns have no reader in this build.
 */

#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <ifaddrs.h>
#include <net/if.h>
#include <netinet/in.h>
#include <sys/socket.h>

#include <linux/netlink.h>
#include <linux/rtnetlink.h>

/* Refuse a single netlink datagram larger than this rather than grow forever. */
#define RD_NL_MAX_BUF (1024 * 1024)
/* A dump that never terminates must not hang the caller. */
#define RD_NL_MAX_DATAGRAMS 4096

typedef int (*rd_nl_cb)(struct nlmsghdr *nlh, void *ctx);

struct rd_link_info {
    unsigned int index;
    unsigned int flags;
    char name[IFNAMSIZ + 1];
};

struct rd_link_table {
    struct rd_link_info *items;
    size_t len;
    size_t cap;
};

/* One allocation per reported address; `ifa` first so freeifaddrs() can free
 * the node it is handed. */
struct rd_ifaddrs_storage {
    struct ifaddrs ifa;
    struct sockaddr_storage addr;
    struct sockaddr_storage netmask;
    struct sockaddr_storage ifu;
    char name[IFNAMSIZ + 1];
};

struct rd_addr_ctx {
    const struct rd_link_table *links;
    struct ifaddrs *head;
    struct ifaddrs *tail;
};

static void rd_parse_rtattr(struct rtattr *rta, int len, struct rtattr **tb, int max)
{
    memset(tb, 0, sizeof(*tb) * ((size_t)max + 1));
    for (; RTA_OK(rta, len); rta = RTA_NEXT(rta, len)) {
        if (rta->rta_type <= (unsigned short)max && tb[rta->rta_type] == NULL)
            tb[rta->rta_type] = rta;
    }
}

/* `len` must stay signed: NLMSG_NEXT subtracts the *aligned* length, which
 * overshoots on an unaligned trailing message, and only a negative remainder
 * stops NLMSG_OK from reading past the buffer. */
static int rd_nl_parse(char *buf, int len, unsigned short reply_type, unsigned int seq,
                       rd_nl_cb cb, void *ctx, int *done)
{
    struct nlmsghdr *nlh = (struct nlmsghdr *)buf;

    for (; NLMSG_OK(nlh, len); nlh = NLMSG_NEXT(nlh, len)) {
        if (nlh->nlmsg_seq != seq)
            continue;
        if (nlh->nlmsg_type == NLMSG_DONE) {
            *done = 1;
            return 0;
        }
        if (nlh->nlmsg_type == NLMSG_ERROR) {
            struct nlmsgerr *err = (struct nlmsgerr *)NLMSG_DATA(nlh);
            if (nlh->nlmsg_len >= NLMSG_LENGTH(sizeof(*err)) && err->error != 0)
                errno = -err->error;
            else
                errno = EIO;
            return -1;
        }
        if (nlh->nlmsg_type != reply_type)
            continue;
        if (cb(nlh, ctx) != 0)
            return -1;
        /* A non-multipart reply is the whole answer; nothing follows it. */
        if ((nlh->nlmsg_flags & NLM_F_MULTI) == 0) {
            *done = 1;
            return 0;
        }
    }
    return 0;
}

static int rd_nl_dump(int fd, unsigned short request_type, unsigned short reply_type,
                      unsigned int seq, rd_nl_cb cb, void *ctx)
{
    struct {
        struct nlmsghdr nlh;
        struct rtgenmsg gen;
    } req;
    struct sockaddr_nl kernel;
    char *buf;
    size_t cap = 8192;
    int datagrams = 0;
    int done = 0;
    int rc = -1;
    int saved;

    memset(&req, 0, sizeof(req));
    req.nlh.nlmsg_len = NLMSG_LENGTH(sizeof(req.gen));
    req.nlh.nlmsg_type = request_type;
    req.nlh.nlmsg_flags = NLM_F_REQUEST | NLM_F_DUMP;
    req.nlh.nlmsg_seq = seq;
    req.gen.rtgen_family = AF_UNSPEC;

    memset(&kernel, 0, sizeof(kernel));
    kernel.nl_family = AF_NETLINK;

    for (;;) {
        if (sendto(fd, &req, req.nlh.nlmsg_len, 0, (struct sockaddr *)&kernel,
                   sizeof(kernel)) >= 0)
            break;
        if (errno != EINTR)
            return -1;
    }

    buf = (char *)malloc(cap);
    if (buf == NULL) {
        errno = ENOMEM;
        return -1;
    }

    while (!done) {
        /* MSG_PEEK|MSG_TRUNC reports the datagram's real size, so an
         * undersized buffer costs a resize instead of a silent truncation. */
        ssize_t n = recv(fd, buf, cap, MSG_PEEK | MSG_TRUNC);
        if (n < 0) {
            if (errno == EINTR)
                continue;
            goto out;
        }
        if ((size_t)n > cap) {
            char *grown;
            if ((size_t)n > RD_NL_MAX_BUF) {
                errno = EMSGSIZE;
                goto out;
            }
            grown = (char *)realloc(buf, (size_t)n);
            if (grown == NULL) {
                errno = ENOMEM;
                goto out;
            }
            buf = grown;
            cap = (size_t)n;
            continue;
        }
        n = recv(fd, buf, cap, 0);
        if (n < 0) {
            if (errno == EINTR)
                continue;
            goto out;
        }
        if (n == 0 || ++datagrams > RD_NL_MAX_DATAGRAMS) {
            errno = EIO;
            goto out;
        }
        if (rd_nl_parse(buf, (int)n, reply_type, seq, cb, ctx, &done) != 0)
            goto out;
    }
    rc = 0;

out:
    saved = errno;
    free(buf);
    errno = saved;
    return rc;
}

static int rd_link_cb(struct nlmsghdr *nlh, void *ctx)
{
    struct rd_link_table *t = (struct rd_link_table *)ctx;
    struct ifinfomsg *ifi;
    struct rtattr *tb[IFLA_IFNAME + 1];
    struct rd_link_info *slot;
    int payload;
    int namelen;

    if (nlh->nlmsg_len < NLMSG_LENGTH(sizeof(*ifi)))
        return 0;
    ifi = (struct ifinfomsg *)NLMSG_DATA(nlh);
    payload = (int)nlh->nlmsg_len - (int)NLMSG_SPACE(sizeof(*ifi));
    if (payload < 0)
        payload = 0;
    rd_parse_rtattr(IFLA_RTA(ifi), payload, tb, IFLA_IFNAME);

    /* An interface we cannot name is of no use: callers dereference ifa_name. */
    if (tb[IFLA_IFNAME] == NULL || (int)RTA_PAYLOAD(tb[IFLA_IFNAME]) <= 0)
        return 0;

    if (t->len == t->cap) {
        size_t ncap = t->cap ? t->cap * 2 : 16;
        struct rd_link_info *items =
            (struct rd_link_info *)realloc(t->items, ncap * sizeof(*items));
        if (items == NULL) {
            errno = ENOMEM;
            return -1;
        }
        t->items = items;
        t->cap = ncap;
    }

    slot = &t->items[t->len];
    memset(slot, 0, sizeof(*slot));
    slot->index = (unsigned int)ifi->ifi_index;
    slot->flags = ifi->ifi_flags;
    namelen = (int)RTA_PAYLOAD(tb[IFLA_IFNAME]);
    if (namelen > IFNAMSIZ)
        namelen = IFNAMSIZ;
    memcpy(slot->name, RTA_DATA(tb[IFLA_IFNAME]), (size_t)namelen);
    slot->name[namelen] = '\0';
    t->len++;
    return 0;
}

static const struct rd_link_info *rd_link_find(const struct rd_link_table *t,
                                               unsigned int index)
{
    size_t i;
    for (i = 0; i < t->len; i++) {
        if (t->items[i].index == index)
            return &t->items[i];
    }
    return NULL;
}

static void rd_fill_mask(unsigned char *out, int len, unsigned int prefix)
{
    int i;
    if (prefix > (unsigned int)len * 8)
        prefix = (unsigned int)len * 8;
    for (i = 0; i < len; i++) {
        if (prefix >= 8) {
            out[i] = 0xff;
            prefix -= 8;
        } else if (prefix > 0) {
            out[i] = (unsigned char)(0xff << (8 - prefix));
            prefix = 0;
        } else {
            out[i] = 0;
        }
    }
}

static void rd_set_in(struct sockaddr_storage *ss, const void *addr)
{
    struct sockaddr_in *sin = (struct sockaddr_in *)ss;
    sin->sin_family = AF_INET;
    memcpy(&sin->sin_addr, addr, 4);
}

static int rd_addr_cb(struct nlmsghdr *nlh, void *ctx)
{
    struct rd_addr_ctx *c = (struct rd_addr_ctx *)ctx;
    struct ifaddrmsg *ifa;
    struct rtattr *tb[IFA_BROADCAST + 1];
    struct rtattr *ra;
    const struct rd_link_info *link;
    struct rd_ifaddrs_storage *st;
    int payload;

    if (nlh->nlmsg_len < NLMSG_LENGTH(sizeof(*ifa)))
        return 0;
    ifa = (struct ifaddrmsg *)NLMSG_DATA(nlh);
    if (ifa->ifa_family != AF_INET && ifa->ifa_family != AF_INET6)
        return 0;

    /* Without the link entry there is no name, and callers deref ifa_name. */
    link = rd_link_find(c->links, ifa->ifa_index);
    if (link == NULL)
        return 0;

    payload = (int)nlh->nlmsg_len - (int)NLMSG_SPACE(sizeof(*ifa));
    if (payload < 0)
        payload = 0;
    rd_parse_rtattr(IFA_RTA(ifa), payload, tb, IFA_BROADCAST);

    /* On a point-to-point link IFA_ADDRESS holds the peer and IFA_LOCAL the
     * local address; ipv6 only ever sets IFA_ADDRESS. */
    if (ifa->ifa_family == AF_INET)
        ra = tb[IFA_LOCAL] ? tb[IFA_LOCAL] : tb[IFA_ADDRESS];
    else
        ra = tb[IFA_ADDRESS] ? tb[IFA_ADDRESS] : tb[IFA_LOCAL];
    if (ra == NULL)
        return 0;
    if ((int)RTA_PAYLOAD(ra) < (ifa->ifa_family == AF_INET ? 4 : 16))
        return 0;

    st = (struct rd_ifaddrs_storage *)calloc(1, sizeof(*st));
    if (st == NULL) {
        errno = ENOMEM;
        return -1;
    }

    memcpy(st->name, link->name, sizeof(st->name));
    st->ifa.ifa_name = st->name;
    st->ifa.ifa_flags = link->flags;
    st->ifa.ifa_addr = (struct sockaddr *)&st->addr;
    st->ifa.ifa_netmask = (struct sockaddr *)&st->netmask;

    if (ifa->ifa_family == AF_INET) {
        struct sockaddr_in *mask = (struct sockaddr_in *)&st->netmask;

        rd_set_in(&st->addr, RTA_DATA(ra));
        mask->sin_family = AF_INET;
        rd_fill_mask((unsigned char *)&mask->sin_addr, 4, ifa->ifa_prefixlen);

        if ((link->flags & IFF_POINTOPOINT) && tb[IFA_ADDRESS] && tb[IFA_LOCAL] &&
            (int)RTA_PAYLOAD(tb[IFA_ADDRESS]) >= 4 &&
            memcmp(RTA_DATA(tb[IFA_ADDRESS]), RTA_DATA(tb[IFA_LOCAL]), 4) != 0) {
            rd_set_in(&st->ifu, RTA_DATA(tb[IFA_ADDRESS]));
            st->ifa.ifa_dstaddr = (struct sockaddr *)&st->ifu;
        } else if (tb[IFA_BROADCAST] && (int)RTA_PAYLOAD(tb[IFA_BROADCAST]) >= 4) {
            rd_set_in(&st->ifu, RTA_DATA(tb[IFA_BROADCAST]));
            st->ifa.ifa_broadaddr = (struct sockaddr *)&st->ifu;
        }
    } else {
        struct sockaddr_in6 *sin6 = (struct sockaddr_in6 *)&st->addr;
        struct sockaddr_in6 *mask = (struct sockaddr_in6 *)&st->netmask;

        sin6->sin6_family = AF_INET6;
        memcpy(&sin6->sin6_addr, RTA_DATA(ra), 16);
        /* A link-local address is not routable without its scope id. */
        if (IN6_IS_ADDR_LINKLOCAL(&sin6->sin6_addr) ||
            IN6_IS_ADDR_MC_LINKLOCAL(&sin6->sin6_addr))
            sin6->sin6_scope_id = ifa->ifa_index;
        mask->sin6_family = AF_INET6;
        rd_fill_mask((unsigned char *)&mask->sin6_addr, 16, ifa->ifa_prefixlen);
    }

    if (c->tail != NULL)
        c->tail->ifa_next = &st->ifa;
    else
        c->head = &st->ifa;
    c->tail = &st->ifa;
    return 0;
}

void freeifaddrs(struct ifaddrs *ifa)
{
    while (ifa != NULL) {
        struct ifaddrs *next = ifa->ifa_next;
        free(ifa);
        ifa = next;
    }
}

int getifaddrs(struct ifaddrs **ifap)
{
    struct rd_link_table links;
    struct rd_addr_ctx ctx;
    int fd;
    int saved;

    if (ifap == NULL) {
        errno = EINVAL;
        return -1;
    }
    *ifap = NULL;

    memset(&links, 0, sizeof(links));
    memset(&ctx, 0, sizeof(ctx));
    ctx.links = &links;

    fd = socket(AF_NETLINK, SOCK_RAW | SOCK_CLOEXEC, NETLINK_ROUTE);
    if (fd < 0)
        return -1;

    if (rd_nl_dump(fd, RTM_GETLINK, RTM_NEWLINK, 1, rd_link_cb, &links) != 0)
        goto fail;
    if (rd_nl_dump(fd, RTM_GETADDR, RTM_NEWADDR, 2, rd_addr_cb, &ctx) != 0)
        goto fail;

    close(fd);
    free(links.items);
    *ifap = ctx.head;
    return 0;

fail:
    saved = errno;
    close(fd);
    free(links.items);
    freeifaddrs(ctx.head);
    errno = saved;
    return -1;
}
