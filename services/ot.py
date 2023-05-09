import json


def isValid(start, end, ots, real):
    current = str(start)
    cursor = 0
    valid = True
    ops = json.loads(ots)
    for op in ops:
        if not valid:
            pass
        elif op["op"] == "skip":
            skip = op["count"]
            if skip + cursor + 1 > len(current):
                valid = False
            cursor += skip
        elif op["op"] == "delete":
            delete = op["count"]
            if delete + cursor > len(current):
                valid = False

            current = current[0:cursor] + current[cursor + delete :]
        elif op["op"] == "insert":
            insert = op["chars"]
            current = current[0:cursor] + insert + current[cursor:]
            cursor += len(insert)

    result = valid and (current == end)
    print(result == real)
