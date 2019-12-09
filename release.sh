#!/bin/bash
set -eux -o pipefail

gothub release -u nytopop -r askii -t ${TAG} -d "$(< ${CHANGELOG})"
