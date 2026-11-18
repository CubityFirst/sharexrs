"""Replaces the rows of a History.db with another's (same ids, same
sqlite_sequence), through SQL so an app holding the database open (ShareX
keeps its connection for its lifetime) reads the new rows on its next load.

    python scripts/history-load.py <fixture History.db> <target History.db>
    python scripts/history-load.py --dump <History.db>   # rows, for comparing

The dump prints every row with each value's SQLite type, as the history
harness compares databases.
"""
import sqlite3
import sys


def dump(path):
    c = sqlite3.connect(path)
    for row in c.execute("SELECT * FROM History ORDER BY Id"):
        print(repr(row))
    for row in c.execute("SELECT name, seq FROM sqlite_sequence"):
        print("seq", row)


def load(src, dst):
    c = sqlite3.connect(dst, isolation_level=None, timeout=30)
    c.execute("ATTACH DATABASE ? AS src", (src,))
    c.execute("BEGIN IMMEDIATE")
    c.execute("DELETE FROM main.History")
    c.execute("INSERT INTO main.History SELECT * FROM src.History ORDER BY Id")
    c.execute("DELETE FROM main.sqlite_sequence WHERE name = 'History'")
    c.execute("INSERT INTO main.sqlite_sequence SELECT * FROM src.sqlite_sequence WHERE name = 'History'")
    c.execute("COMMIT")
    n = c.execute("SELECT COUNT(*) FROM main.History").fetchone()[0]
    print(f"{dst}: {n} rows")


if __name__ == "__main__":
    if sys.argv[1] == "--dump":
        dump(sys.argv[2])
    else:
        load(sys.argv[1], sys.argv[2])
