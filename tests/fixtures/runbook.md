# Sample runbook

Press `Tab` to focus a block and `Enter` to run it. `R` runs every block from the focused one.

## Check the environment

```sh {name=env}
echo "shell: $SHELL"
uname -s
```

## Count to three slowly

```bash {name=count}
for i in 1 2 3; do echo "tick $i"; sleep 0.5; done
```

## Fail on purpose

```sh {name=fail}
echo "about to fail" >&2
exit 4
```

## Never runs

```sh {norun}
rm -rf /tmp/should-not-run
```

## Python

```python
print(sum(range(10)))
```
