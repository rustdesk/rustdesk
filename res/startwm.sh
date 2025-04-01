#!/usr/bin/env bash

# This script is derived from https://github.com/neutrinolabs/xrdp/sesman/startwm.sh.

#
# This script is an example. You might need to edit this script
# depending on your distro if it doesn't work for you.
#
# Uncomment the following line for debug:
# exec xterm


# Execution sequence for interactive login shell - pseudocode
#
# IF /etc/profile is readable THEN
#     execute ~/.bash_profile
# END IF
# IF ~/.bash_profile is readable THEN
#     execute ~/.bash_profile
# ELSE
#     IF ~/.bash_login is readable THEN
#         execute ~/.bash_login
#     ELSE
#         IF ~/.profile is readable THEN
#             execute ~/.profile
#         END IF
#     END IF
# END IF
pre_start()
{
  if [ -r /etc/profile ]; then
    . /etc/profile
  fi
  if [ -r ~/.bash_profile ]; then
    . ~/.bash_profile
  else
    if [ -r ~/.bash_login ]; then
      . ~/.bash_login
    else
      if [ -r ~/.profile ]; then
        . ~/.profile
      fi
    fi
  fi
  return 0
}

# When loging out from the interactive shell, the execution sequence is:
#
# IF ~/.bash_logout exists THEN
#     execute ~/.bash_logout
# END IF
post_start()
{
  if [ -r ~/.bash_logout ]; then
    . ~/.bash_logout
  fi
  return 0
}

#start the window manager
wm_start()
{
  if [ -r /etc/default/locale ]; then
    . /etc/default/locale
    export LANG LANGUAGE
  fi

  # debian
  if [ -r /etc/X11/Xsession ]; then
    pre_start
    . /etc/X11/Xsession
    post_start
    exit 0
  fi

  # alpine
  # Don't use /etc/X11/xinit/Xsession - it doesn't work
  if [ -f /etc/alpine-release ]; then
    if [ -f /etc/X11/xinit/xinitrc ]; then
        pre_start
        /etc/X11/xinit/xinitrc
        post_start
    else
        echo "** xinit package isn't installed" >&2
        exit 1
    fi
  fi

  # el
  if [ -r /etc/X11/xinit/Xsession ]; then
    pre_start
    . /etc/X11/xinit/Xsession
    post_start
    exit 0
  fi

  # suse
  if [ -r /etc/X11/xdm/Xsession ]; then
    # since the following script run a user login shell,
    # do not execute the pseudo login shell scripts
    . /etc/X11/xdm/Xsession
    exit 0
  elif [ -r /usr/etc/X11/xdm/Xsession ]; then
    . /usr/etc/X11/xdm/Xsession
    exit 0
  fi

  pre_start
  xterm
  post_start
}

#. /etc/environment
#export PATH=$PATH
#export LANG=$LANG

# change PATH to be what your environment needs usually what is in
# /etc/environment
#PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/usr/games"
#export PATH=$PATH

# for PATH and LANG from /etc/environment
# pam will auto process the environment file if /etc/pam.d/xrdp-sesman
# includes
# auth       required     pam_env.so readenv=1

wm_start

exit 1
