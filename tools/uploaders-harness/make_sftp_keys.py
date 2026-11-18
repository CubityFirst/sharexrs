"""Writes the SFTP test keys in every format SSH.NET's PrivateKeyFile reads
(crates/sharex-uploaders/tests/fixtures/sftp_test_*), each with its .pub.
Made up for the tests; never used anywhere else. Needs OpenSSL 3.2+ (for
Argon2) and ssh-keygen.

  python tools/uploaders-harness/make_sftp_keys.py
"""

import base64
import hashlib
import hmac
import os
import struct
import subprocess
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "..", "crates", "sharex-uploaders", "tests", "fixtures")
PASS = "made-up-passphrase"


def run(*args, data=None):
    return subprocess.run(args, input=data, capture_output=True, check=True).stdout


def write(name, text, pub):
    with open(os.path.join(OUT, name), "w", newline="\n") as f:
        f.write(text)
    with open(os.path.join(OUT, name + ".pub"), "w", newline="\n") as f:
        f.write(pub.strip() + "\n")


def pub_of(path, passphrase=""):
    return run("ssh-keygen", "-y", "-P", passphrase, "-f", path).decode()


def ssh_string(b):
    if isinstance(b, str):
        b = b.encode()
    return struct.pack(">I", len(b)) + b


def ssh_mpint(n):
    b = n.to_bytes((n.bit_length() + 7) // 8 or 1, "big")
    if b[0] & 0x80:
        b = b"\0" + b
    return ssh_string(b)


def aes256_cbc(data, key, iv):
    return run("openssl", "enc", "-aes-256-cbc", "-nopad", "-K", key.hex(), "-iv", iv.hex(), data=data)


def des3_cbc(data, key, iv):
    return run("openssl", "enc", "-des-ede3-cbc", "-nopad", "-K", key.hex(), "-iv", iv.hex(), data=data)


def argon2id(passphrase, salt, memory, passes, lanes, length):
    out = run("openssl", "kdf", "-keylen", str(length), "-kdfopt", f"pass:{passphrase}", "-kdfopt", f"hexsalt:{salt.hex()}",
              "-kdfopt", f"iter:{passes}", "-kdfopt", f"memcost:{memory}", "-kdfopt", f"lanes:{lanes}", "-kdfopt", "threads:1",
              "ARGON2ID").decode()
    return bytes.fromhex(out.strip().replace(":", ""))


def lines(b, width):
    s = base64.b64encode(b).decode()
    return [s[i:i + width] for i in range(0, len(s), width)]


def rsa_numbers(pem_path):
    """n, e, d, p, q, iqmp of an RSA key, from `openssl rsa -text`."""
    text = run("openssl", "rsa", "-in", pem_path, "-noout", "-text").decode()
    fields = {}
    name = None
    for line in text.splitlines():
        if line and not line.startswith(" ") and ":" in line:
            key = line.split(":")[0].strip()
            rest = line.split(":", 1)[1].strip()
            name = key
            fields[name] = rest
        elif name:
            fields[name] = fields[name] + line.strip()
    def num(k):
        v = fields[k]
        if "(" in v:
            return int(v.split("(")[1].split(")")[0].replace("0x", ""), 16) if "0x" in v else int(v.split(" ")[0])
        return int(v.replace(":", ""), 16)
    return {"n": num("modulus"), "e": num("publicExponent"), "d": num("privateExponent"), "p": num("prime1"), "q": num("prime2"),
            "iqmp": num("coefficient")}


def ppk(version, algorithm, public_blob, private_blob, comment, encrypted):
    enc = "aes256-cbc" if encrypted else "none"
    if encrypted:
        # Padded to the cipher's block (the MAC covers the padding).
        private_blob += os.urandom((-len(private_blob)) % 16)
    kdf_lines = []
    if version == 3:
        if encrypted:
            salt = bytes(range(16))
            memory, passes, lanes = 8192, 8, 1
            derived = argon2id(PASS, salt, memory, passes, lanes, 80)
            key, iv, mac_key = derived[:32], derived[32:48], derived[48:80]
            kdf_lines = ["Key-Derivation: Argon2id", f"Argon2-Memory: {memory}", f"Argon2-Passes: {passes}", f"Argon2-Parallelism: {lanes}",
                         f"Argon2-Salt: {salt.hex()}"]
        else:
            mac_key = b""
        mac_hash = hashlib.sha256
    else:
        if encrypted:
            seq = lambda i: hashlib.sha1(struct.pack(">I", i) + PASS.encode()).digest()
            key, iv = (seq(0) + seq(1))[:32], bytes(16)
            mac_key = hashlib.sha1(("putty-private-key-file-mac-key" + PASS).encode()).digest()
        else:
            mac_key = hashlib.sha1(b"putty-private-key-file-mac-key").digest()
        mac_hash = hashlib.sha1
    mac_data = ssh_string(algorithm) + ssh_string(enc) + ssh_string(comment) + ssh_string(public_blob) + ssh_string(private_blob)
    mac = hmac.new(mac_key, mac_data, mac_hash).hexdigest()
    stored = aes256_cbc(private_blob, key, iv) if encrypted else private_blob
    pub_lines = lines(public_blob, 64)
    priv_lines = lines(stored, 64)
    out = [f"PuTTY-User-Key-File-{version}: {algorithm}", f"Encryption: {enc}", f"Comment: {comment}", f"Public-Lines: {len(pub_lines)}",
           *pub_lines, *kdf_lines, f"Private-Lines: {len(priv_lines)}", *priv_lines, f"Private-MAC: {mac}"]
    return "\n".join(out) + "\n"


def main():
    tmp = tempfile.mkdtemp()
    p = lambda n: os.path.join(tmp, n)
    # Traditional PEM RSA (PKCS#1), encrypted with AES-128-CBC and with 3DES.
    run("openssl", "genrsa", "-traditional", "-out", p("rsa"), "2048")
    run("openssl", "rsa", "-in", p("rsa"), "-traditional", "-aes128", "-passout", f"pass:{PASS}", "-out", p("rsa_aes"))
    run("openssl", "rsa", "-in", p("rsa"), "-traditional", "-des3", "-passout", f"pass:{PASS}", "-out", p("rsa_3des"))
    rsa_pub = pub_of(p("rsa"))
    write("sftp_test_rsa_pem_aes", open(p("rsa_aes")).read(), rsa_pub)
    write("sftp_test_rsa_pem_3des", open(p("rsa_3des")).read(), rsa_pub)
    # Encrypted PKCS#8 RSA (PBES2).
    run("openssl", "pkcs8", "-topk8", "-in", p("rsa"), "-v2", "aes-256-cbc", "-passout", f"pass:{PASS}", "-out", p("rsa_p8"))
    write("sftp_test_rsa_pkcs8_enc", open(p("rsa_p8")).read(), rsa_pub)
    # SEC1 EC P-384, encrypted; PKCS#8 P-256, plain; ed25519, encrypted PKCS#8.
    run("openssl", "ecparam", "-name", "secp384r1", "-genkey", "-noout", "-out", p("ec384"))
    run("openssl", "ec", "-in", p("ec384"), "-aes256", "-passout", f"pass:{PASS}", "-out", p("ec384_enc"))
    write("sftp_test_ec384_pem", open(p("ec384_enc")).read(), pub_of(p("ec384")))
    run("openssl", "genpkey", "-algorithm", "EC", "-pkeyopt", "ec_paramgen_curve:P-256", "-out", p("ec256_p8"))
    write("sftp_test_ec256_pkcs8", open(p("ec256_p8")).read(), pub_of(p("ec256_p8")))
    run("openssl", "genpkey", "-algorithm", "ed25519", "-out", p("ed_p8"))
    run("openssl", "pkcs8", "-topk8", "-in", p("ed_p8"), "-v2", "aes-256-cbc", "-passout", f"pass:{PASS}", "-out", p("ed_p8_enc"))
    write("sftp_test_ed25519_pkcs8_enc", open(p("ed_p8_enc")).read(), pub_of(p("ed_p8")))
    # OpenSSH ECDSA P-521.
    run("ssh-keygen", "-q", "-t", "ecdsa", "-b", "521", "-N", "", "-C", "made-up", "-f", p("ec521"))
    write("sftp_test_ec521", open(p("ec521")).read(), open(p("ec521.pub")).read())
    # PuTTY: v2 encrypted RSA, v3 encrypted RSA (Argon2id), v3 plain RSA.
    r = rsa_numbers(p("rsa"))
    pub_blob = ssh_string("ssh-rsa") + ssh_mpint(r["e"]) + ssh_mpint(r["n"])
    priv_blob = ssh_mpint(r["d"]) + ssh_mpint(r["p"]) + ssh_mpint(r["q"]) + ssh_mpint(r["iqmp"])
    write("sftp_test_rsa_v2.ppk", ppk(2, "ssh-rsa", pub_blob, priv_blob, "made-up-key", True), rsa_pub)
    write("sftp_test_rsa_v3.ppk", ppk(3, "ssh-rsa", pub_blob, priv_blob, "made-up-key", True), rsa_pub)
    write("sftp_test_rsa_v3_plain.ppk", ppk(3, "ssh-rsa", pub_blob, priv_blob, "made-up-key", False), rsa_pub)
    # SSH.COM (SSH2 ENCRYPTED PRIVATE KEY), RSA, 3des-cbc.
    def bits(n):
        b = n.to_bytes((n.bit_length() + 7) // 8, "big")
        return struct.pack(">I", n.bit_length()) + b
    body = bits(r["e"]) + bits(r["d"]) + bits(r["n"]) + bits(r["iqmp"]) + bits(r["q"]) + bits(r["p"])
    plain = struct.pack(">I", len(body)) + body
    plain += b"\0" * ((-len(plain)) % 8)
    key = b""
    h = hashlib.md5(PASS.encode()).digest()
    key += h
    while len(key) < 24:
        h = hashlib.md5(PASS.encode() + h).digest()
        key += h
    encrypted = des3_cbc(plain, key[:24], bytes(8))
    key_type = "if-modn{sign{rsa-pkcs1-sha1},encrypt{rsa-pkcs1v2-oaep}}"
    blob = ssh_string(key_type) + ssh_string("3des-cbc") + ssh_string(encrypted)
    blob = struct.pack(">II", 0x3F6FF9EB, len(blob) + 8) + blob
    sshcom = "---- BEGIN SSH2 ENCRYPTED PRIVATE KEY ----\nComment: \"made-up-key\"\n" + "\n".join(lines(blob, 70)) + \
             "\n---- END SSH2 ENCRYPTED PRIVATE KEY ----\n"
    write("sftp_test_rsa_sshcom", sshcom, rsa_pub)
    print("keys written to", os.path.normpath(OUT))


if __name__ == "__main__":
    main()
