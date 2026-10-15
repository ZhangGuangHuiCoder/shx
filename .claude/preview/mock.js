(function () {
  const q = new URLSearchParams(location.search);
  if (!localStorage.getItem("shx.profiles") || q.get("reset")) {
    localStorage.setItem("shx.profiles", JSON.stringify([{ id: "a1", name: "h.ofis.cn", group: "web", host: "h.ofis.cn", port: 22, username: "root", auth: "key", keyId: "k2", saveSecret: true, note: "", keepAlive: false }]));
    localStorage.setItem("shx.keys", JSON.stringify([{ id: "k1", name: "id_ed25519", content: "x", encrypted: false }, { id: "k2", name: "deploy_rsa", content: "x", encrypted: true }, { id: "k3", name: "github", content: "x", encrypted: false }]));
    localStorage.setItem("shx.settings", JSON.stringify({ theme: q.get("theme") || "light" }));
  }
  let cb = 1;
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
    transformCallback(fn) { const id = cb++; window["_" + id] = fn; return id; },
    unregisterCallback() {}, convertFileSrc: (s) => s,
    async invoke(cmd, args) {
      switch (cmd) {
        case "ssh_connect": return "s1";
        case "sftp_list": {
          const dir = ["~", ".", ""].includes(args.path || "") ? "/root" : args.path;
          const mk = (name, isDir) => ({ name, path: (dir === "/" ? "" : dir) + "/" + name, kind: isDir ? "dir" : "file", isDir, size: isDir ? 0 : 1234, mtime: 1760000000, mode: isDir ? 0o40755 : 0o100644, uid: 0, gid: 0, owner: "root", group: "root", linkTarget: null });
          const tree = {
            "/": [mk("etc", 1), mk("root", 1), mk("var", 1)],
            "/root": [mk(".ssh", 1), mk("app", 1), mk(".bashrc"), mk(".npmrc"), mk(".gitconfig"), mk("deploy.sh"), mk("notes.md")],
            "/root/app": [mk("src", 1), mk("package.json"), mk("server.py"), mk("config.yaml")],
            "/root/app/src": [mk("main.ts"), mk("util.ts")],
          };
          return { path: dir, entries: tree[dir] || [] };
        }
        case "sftp_read_text": {
          const name = args.path.split("/").pop();
          const samples = {
            ".npmrc": "registry=https://registry.npmmirror.com/\nlockfile=false\nauto-install-peers=true\nprefix=/www/server/nodejs/v22.19.0/\nstrict-ssl=false\n",
            "deploy.sh": "#!/usr/bin/env bash\nset -euo pipefail\n\ncd /root/app\ngit pull --ff-only\nnpm ci\nsystemctl restart app\n",
            "server.py": "from http.server import HTTPServer, SimpleHTTPRequestHandler\n\n\ndef main():\n    HTTPServer((\"\", 8000), SimpleHTTPRequestHandler).serve_forever()\n\n\nif __name__ == \"__main__\":\n    main()\n",
          };
          return { content: samples[name] ?? "# " + name + "\n", writable: name !== ".gitconfig" };
        }
        case "sftp_write_text": return null;
        case "keepalive_prepare": return { ready: true, fresh: true };
        case "term_open": {
          const ch = args && (args.onData || args.onOutput || Object.values(args).find((v) => v && v.onmessage !== undefined));
          setTimeout(() => ch && ch.onmessage && ch.onmessage([...new TextEncoder().encode("[root@alma www]# ls\r\nbin  etc  \x1b[34musr\x1b[0m\r\n[root@alma www]# ")]), 300);
          return null;
        }
        case "transfer_list": return [];
        case "keepalive_list": return [];
        case "plugin:event|listen": return cb++;
        case "plugin:menu|new": return [cb++, "m" + cb];
        case "plugin:window|is_visible": return true;
        default: return cmd.startsWith("plugin:menu|") ? [] : cmd.includes("is_") ? false : null;
      }
    },
  };
})();
