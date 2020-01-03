#!/bin/bash
set -eux -o pipefail

if [[ ${TAG} =~ "pre" ]]; then
	gothub release -p -u nytopop -r askii -t ${TAG} -d "$(< ${CHANGELOG})"
else
	gothub release -u nytopop -r askii -t ${TAG} -d "$(< ${CHANGELOG})"
fi
