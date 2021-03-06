#!/usr/bin/env python3.7

"""
Pulling available repositories with a valid configuration file should succeed
"""

from nesttests import *

with nest_server(), create_config() as config_path:
    assert nest(config=config_path).pull().returncode == 0
