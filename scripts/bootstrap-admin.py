#!/usr/bin/env python3
"""Provision the first administrator. Credentials travel over stdin, never argv/env."""
import argparse
import getpass
import json
import subprocess
import sys
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--local", action="store_true", help="Use native Cargo instead of Compose")
parser.add_argument("--storage", choices=("postgres", "dynamodb"), help="Override the saved setup storage choice")
args = parser.parse_args()
storage = args.storage
if storage is None:
    config = Path("appshell.deploy.json")
    storage = json.loads(config.read_text()).get("storage", "postgres") if config.exists() else "postgres"
if storage not in ("postgres", "dynamodb"):
    sys.exit("Invalid saved storage choice. Run setup or specify --storage.")

email = input("Administrator email: ").strip()
name = input("Administrator name: ").strip()
password = getpass.getpass("Password (12–128 bytes): ")
if password != getpass.getpass("Confirm password: "):
    sys.exit("Passwords do not match.")
command = ["cargo", "run", "--locked", "--bin", "bootstrap-admin"]
if storage == "dynamodb":
    command += ["--no-default-features", "--features", "dynamodb"]
if not args.local:
    compose = ["docker", "compose"]
    if storage == "dynamodb":
        compose += ["-f", "compose.dynamodb.yaml"]
    command = compose + ["run", "--rm", "-T", "--no-deps", "api"] + command
result = subprocess.run(command, input=json.dumps({"email": email, "name": name, "password": password}), text=True, check=False)
sys.exit(result.returncode)
