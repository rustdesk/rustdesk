#! /bin/sh

args=$(getopt bfos "$@")
if [ $? -ne 0 ]; then
  echo "Usage: autogen.sh [-b] [-f] [-o] [-s] [--]"
  echo
  echo ">   -b: do not update the system detection scripts"
  echo ">   -f: force the recreation of all autoconf scripts"
  echo ">   -o: overwrite/downgrade system detection scripts"
  echo ">   -s: setup an environment for developers"
  exit 2
fi

force=false
update_config=true
overwrite_config=false
dev_setup=false

eval set -- "$args"

while [ $# -ne 0 ]; do
  case $1 in
  -b)
    update_config=false
    ;;
  -f)
    force=true
    ;;
  -o)
    overwrite_config=true
    ;;
  -s)
    dev_setup=true
    ;;
  --)
    shift
    break
    ;;
  esac
  shift
done

if [ -s configure ]; then
  if [ "$force" != true ]; then
    echo "autoconf scripts already exist." >&2
    exit 0
  fi
elif [ "$dev_setup" != true ]; then
  echo "A development environment was not created."
  exit 0
fi

if glibtoolize --version >/dev/null 2>&1; then
  LIBTOOLIZE='glibtoolize'
else
  LIBTOOLIZE='libtoolize'
fi

command -v command >/dev/null 2>&1 || {
  echo "command is required, but wasn't found on this system"
  exit 1
}

command -v $LIBTOOLIZE >/dev/null 2>&1 || {
  echo "libtool is required, but wasn't found on this system"
  exit 1
}

command -v autoconf >/dev/null 2>&1 || {
  echo "autoconf is required, but wasn't found on this system"
  exit 1
}

command -v automake >/dev/null 2>&1 || {
  echo "automake is required, but wasn't found on this system"
  exit 1
}

if [ "$overwrite_config" = false ]; then
  if [ -f build-aux/config.guess ]; then
    mv build-aux/config.guess build-aux/config.guess.stable
  fi
  if [ -f build-aux/config.sub ]; then
    mv build-aux/config.sub build-aux/config.sub.stable
  fi
fi
$LIBTOOLIZE --copy --install &&
  aclocal &&
  automake --add-missing --copy --force-missing --include-deps &&
  autoconf && echo Done.
if [ "$overwrite_config" = false ]; then
  if [ -f build-aux/config.guess.stable ]; then
    mv build-aux/config.guess.stable build-aux/config.guess
  fi
  if [ -f build-aux/config.sub.stable ]; then
    mv build-aux/config.sub.stable build-aux/config.sub
  fi
fi

[ "$update_config" = true ] && [ -z "$DO_NOT_UPDATE_CONFIG_SCRIPTS" ] &&
  command -v curl >/dev/null 2>&1 && {
  echo "Downloading config.guess and config.sub..."

  curl -sSL --fail -o config.guess \
    'https://git.savannah.gnu.org/gitweb/?p=config.git;a=blob_plain;f=config.guess;hb=HEAD' &&
    mv -f config.guess build-aux/config.guess

  curl -sSL --fail -o config.sub \
    'https://git.savannah.gnu.org/gitweb/?p=config.git;a=blob_plain;f=config.sub;hb=HEAD' &&
    mv -f config.sub build-aux/config.sub

  echo "Done."
}

rm -f config.guess config.sub
