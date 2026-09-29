#!/usr/bin/env python3
"""Provision the first administrator. Credentials travel over stdin, never argv/env."""
import getpass
import json
import subprocess
import sys

email = input("Administrator email: ").strip()
name = input("Administrator name: ").strip()
password = getpass.getpass("Password (12–128 bytes): ")
if password != getpass.getpass("Confirm password: "):
    sys.exit("Passwords do not match.")
command = ["cargo", "run", "--locked", "--bin", "bootstrap-admin"]
if "--local" not in sys.argv:
    command = ["docker", "compose", "run", "--rm", "-T", "--no-deps", "api"] + command
result = subprocess.run(command, input=json.dumps({"email": email, "name": name, "password": password}), text=True, check=False)
sys.exit(result.returncode)
