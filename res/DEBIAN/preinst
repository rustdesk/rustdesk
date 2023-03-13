#!/bin/bash

set -e

case $1 in
    install|upgrade)
		INITSYS=$(ls -al /proc/1/exe | awk -F' ' '{print $NF}' | awk -F'/' '{print $NF}')
		if [ "systemd" == "${INITSYS}" ]; then
			service rustdesk stop || true
      sleep 1
      rm -rf /usr/bin/libsciter-gtk.so
		fi
        ;;
esac

exit 0
