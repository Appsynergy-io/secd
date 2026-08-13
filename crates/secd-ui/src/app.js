const LAST_KEY = "secd.last";
const BREAKPOINT = 900;
const FAIL = "That email and credential do not match.";
const EMAIL_AC = "username webauthn";
const PRF_SALT = new Uint8Array(32);
new TextEncoder().encodeInto("secd-prf-kek-v1", PRF_SALT);

const state = {
  session: null,
  remember: null,
  method: null,
  revealPassword: false,
  useDifferent: false,
  screen: "boot",
  width: window.innerWidth,
  userCode: "",
  ephPub: "",
  items: [],
  selected: null,
  wizard: false,
  sessions: [],
  passkeys: [],
  events: [],
  error: "",
  dek: null,
};

function el(tag, attrs, kids) {
  const n = document.createElement(tag);
  if (attrs) {
    for (const [k, v] of Object.entries(attrs)) {
      if (v === false || v === null || v === undefined) continue;
      if (k === "className") n.className = v;
      else if (k.startsWith("on") && typeof v === "function") n.addEventListener(k.slice(2).toLowerCase(), v);
      else if (k === "disabled") n.disabled = !!v;
      else n.setAttribute(k, v === true ? "" : String(v));
    }
  }
  if (kids) for (const c of kids) n.append(c && c.nodeType ? c : document.createTextNode(c == null ? "" : String(c)));
  return n;
}

function rememberFresh(at) {
  const t = Date.parse(at);
  if (Number.isNaN(t)) return false;
  return Date.now() - t <= 30 * 24 * 60 * 60 * 1000;
}

function loadRemember() {
  try {
    const raw = localStorage.getItem(LAST_KEY);
    if (!raw) return null;
    const o = JSON.parse(raw);
    if (!o || typeof o.email !== "string") return null;
    return { email: o.email, has_passkey: !!o.has_passkey, at: String(o.at || "") };
  } catch {
    return null;
  }
}

function saveRemember(email, has_passkey) {
  localStorage.setItem(LAST_KEY, JSON.stringify({
    email,
    has_passkey,
    at: new Date().toISOString(),
  }));
}

function clearRemember() {
  localStorage.removeItem(LAST_KEY);
}

function layoutMode(w) {
  return w >= BREAKPOINT ? "list-inspector" : "list-only";
}

function qs() {
  const u = new URL(location.href);
  return { user_code: u.searchParams.get("user_code") || u.searchParams.get("code") || "", eph_pub: u.searchParams.get("eph_pub") || "" };
}

async function req(method, url, body) {
  const opt = { method, credentials: "same-origin", headers: {} };
  if (body !== undefined) {
    opt.headers["Content-Type"] = "application/json";
    opt.body = JSON.stringify(body);
  }
  const res = await fetch(url, opt);
  let data = {};
  try { data = await res.json(); } catch { data = {}; }
  return { status: res.status, data };
}

function resolveGate() {
  if (state.session) {
    return {
      kind: "approve",
      showEmail: false,
      showPassword: false,
      showPasskey: false,
      showApprove: true,
      emailAc: null,
      prefill: "",
      different: false,
      usePassword: false,
    };
  }
  const r = state.remember;
  const fresh = r && !state.useDifferent && rememberFresh(r.at);
  if (fresh && r.has_passkey) {
    return {
      kind: "remembered-passkey",
      showEmail: false,
      showPassword: false,
      showPasskey: true,
      showApprove: false,
      emailAc: null,
      prefill: r.email,
      different: true,
      usePassword: false,
    };
  }
  if (fresh && !r.has_passkey) {
    return {
      kind: "remembered-password",
      showEmail: false,
      showPassword: true,
      showPasskey: false,
      showApprove: false,
      emailAc: null,
      prefill: r.email,
      different: true,
      usePassword: false,
    };
  }
  if (state.method) {
    const m = state.method;
    let showPassword = false;
    let showPasskey = false;
    let usePassword = false;
    if (m === "passkey") showPasskey = true;
    else if (m === "password") showPassword = true;
    else if (m === "either") {
      showPasskey = true;
      showPassword = state.revealPassword;
      usePassword = !state.revealPassword;
    } else if (m === "register") {
      showPassword = true;
      showPasskey = true;
    }
    return {
      kind: "identity",
      showEmail: true,
      showPassword,
      showPasskey,
      showApprove: false,
      emailAc: EMAIL_AC,
      prefill: r ? r.email : "",
      different: !!r,
      usePassword,
    };
  }
  return {
    kind: "cold",
    showEmail: true,
    showPassword: false,
    showPasskey: true,
    showApprove: false,
    emailAc: EMAIL_AC,
    prefill: r ? r.email : "",
    different: false,
    usePassword: false,
  };
}

function b64urlToBuf(s) {
  if (s instanceof ArrayBuffer) return s;
  if (ArrayBuffer.isView(s)) return s.buffer;
  const str = String(s).replace(/-/g, "+").replace(/_/g, "/");
  const pad = "=".repeat((4 - (str.length % 4)) % 4);
  const bin = atob(str + pad);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out.buffer;
}

function bufToB64url(buf) {
  const u = new Uint8Array(buf);
  let s = "";
  for (const b of u) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

function coercePk(pk) {
  const o = JSON.parse(JSON.stringify(pk));
  o.challenge = b64urlToBuf(o.challenge);
  if (o.user && o.user.id) o.user.id = b64urlToBuf(o.user.id);
  if (o.excludeCredentials) {
    for (const c of o.excludeCredentials) c.id = b64urlToBuf(c.id);
  }
  if (o.allowCredentials) {
    for (const c of o.allowCredentials) c.id = b64urlToBuf(c.id);
  }
  o.extensions = Object.assign({}, o.extensions, { prf: { eval: { first: PRF_SALT } } });
  return o;
}

function serializeCred(cred) {
  const r = cred.response;
  const out = {
    id: cred.id,
    rawId: bufToB64url(cred.rawId),
    type: cred.type,
    response: {},
  };
  if (r.attestationObject) out.response.attestationObject = bufToB64url(r.attestationObject);
  if (r.clientDataJSON) out.response.clientDataJSON = bufToB64url(r.clientDataJSON);
  if (r.authenticatorData) out.response.authenticatorData = bufToB64url(r.authenticatorData);
  if (r.signature) out.response.signature = bufToB64url(r.signature);
  if (r.userHandle) out.response.userHandle = bufToB64url(r.userHandle);
  return out;
}

function prfFromCred(cred) {
  const ext = cred.getClientExtensionResults && cred.getClientExtensionResults();
  const first = ext && ext.prf && ext.prf.results && ext.prf.results.first;
  if (!first) return null;
  const u = new Uint8Array(first);
  if (u.length < 32) return null;
  let hex = "";
  for (const b of u.subarray(0, 32)) hex += b.toString(16).padStart(2, "0");
  for (let i = 0; i < u.length; i++) u[i] = 0;
  return hex;
}

function publicKeyOf(data) {
  if (data && data.publicKey) return data.publicKey;
  return data;
}

async function passkeyCreate(email) {
  const start = await req("POST", "/api/auth/passkey/register/start", { email });
  if (start.status !== 200) return start;
  const pk = coercePk(publicKeyOf(start.data));
  const cred = await navigator.credentials.create({ publicKey: pk });
  const prf = prfFromCred(cred);
  if (!prf) return { status: 400, data: { error: "prf" } };
  return req("POST", "/api/auth/passkey/register/finish", {
    handle: start.data.handle,
    credential: serializeCred(cred),
    prf,
    email,
  });
}

async function passkeyGet(email, conditional) {
  const body = email ? { email } : {};
  const start = await req("POST", "/api/auth/passkey/login/start", body);
  if (start.status !== 200) return start;
  const pk = coercePk(publicKeyOf(start.data));
  const opts = { publicKey: pk };
  if (conditional) opts.mediation = "conditional";
  const cred = await navigator.credentials.get(opts);
  const prf = prfFromCred(cred);
  if (!prf) return { status: 400, data: { error: "prf" } };
  return req("POST", "/api/auth/passkey/login/finish", {
    handle: start.data.handle,
    credential: serializeCred(cred),
    prf,
  });
}

function nav(current) {
  const items = [
    ["register", "Register"],
    ["activity", "Activity"],
    ["account", "Account"],
  ];
  return items.map(([id, label]) =>
    el("a", {
      href: "#" + id,
      "data-nav-item": id,
      "data-current": current === id ? "true" : null,
      onClick: (e) => { e.preventDefault(); go(id); },
    }, [label])
  );
}

function chrome(screen, inner) {
  const layout = layoutMode(state.width);
  return el("div", { className: "app", "data-screen": screen, "data-layout": layout }, [
    el("nav", { className: "nav-top", "data-nav": "utility" }, nav(screen)),
    el("main", { className: "main" }, [inner]),
    el("nav", { className: "nav-bottom", "data-nav": "bottom" }, nav(screen)),
  ]);
}

function errNode() {
  return state.error ? el("p", { className: "err" }, [state.error]) : null;
}

function devicePage() {
  const box = el("input", { type: "text", name: "user_code", autocomplete: "off", value: state.userCode });
  return el("section", { "data-page": "device" }, [
    el("h1", null, ["Approve this machine"]),
    errNode(),
    el("label", { className: "field-label" }, ["Device code", box]),
    el("button", {
      type: "button",
      className: "primary",
      "data-action": "approve",
      onClick: () => approve(box.value.trim()),
    }, ["Approve"]),
  ]);
}

function gatePage(g) {
  const kids = [el("h1", null, ["secd"]), errNode()];
  if (g.showEmail) {
    kids.push(el("label", { className: "field-label" }, [
      "Email",
      el("input", { type: "email", name: "email", autocomplete: g.emailAc || "username", value: g.prefill || "" }),
    ]));
  }
  if (g.showPassword) {
    kids.push(el("label", { className: "field-label" }, [
      "Password",
      el("input", { type: "password", name: "password", autocomplete: "current-password" }),
    ]));
  }
  if (g.showPasskey) {
    kids.push(el("button", { type: "button", className: "primary", "data-action": "passkey", onClick: onPasskey }, ["Use a passkey"]));
  }
  if (g.showPassword) {
    kids.push(el("button", { type: "button", className: "primary", "data-action": "continue", onClick: onPassword }, ["Continue"]));
  }
  if (g.usePassword) {
    kids.push(el("button", { type: "button", className: "ghost", "data-action": "use-password", onClick: () => { state.revealPassword = true; paint(); } }, ["Use a password instead"]));
  }
  if (g.different) {
    kids.push(el("button", { type: "button", className: "ghost", "data-action": "different", onClick: () => { state.useDifferent = true; clearRemember(); state.remember = null; state.method = null; paint(); startConditional(); } }, ["Use a different account"]));
  }
  return el("section", { "data-page": "gate" }, kids.filter(Boolean));
}

function fieldRow(key) {
  return el("div", { className: "field", "data-field": key }, [
    el("span", { className: "name" }, [key]),
    el("button", { type: "button", className: "primary", "data-action": "copy", onClick: () => copyField(key) }, ["Copy"]),
    el("button", { type: "button", className: "ghost", "data-action": "show", "data-hold": "1", onPointerdown: () => showField(key, true), onPointerup: () => showField(key, false), onPointerleave: () => showField(key, false) }, ["Show"]),
    el("span", { className: "value", hidden: true, "data-value": key }),
  ]);
}

function inspector(item) {
  if (!item) return el("aside", { className: "inspector", "data-pane": "inspector" }, [el("p", { className: "muted" }, ["Select a secret"])]);
  return el("aside", { className: "inspector", "data-pane": "inspector" }, [
    el("h2", { className: "name" }, [item.name]),
    ...item.fields.map((f) => fieldRow(f.key)),
  ]);
}

function sheet(item) {
  return el("div", { className: "sheet", "data-sheet": "open", "data-pane": "sheet" }, [
    el("h2", { className: "name" }, [item.name]),
    ...item.fields.map((f) => fieldRow(f.key)),
    el("button", { type: "button", className: "ghost", "data-action": "close-sheet", onClick: () => { state.selected = null; paint(); } }, ["Close"]),
  ]);
}

function wizard() {
  const providers = ["cloudflare","aws","s3","github","gitea","gitlab","slack","digitalocean","npm","xai","sendgrid","pypi","anthropic","openai","vault"];
  return el("section", { className: "wizard", "data-wizard": "open" }, [
    el("h1", null, ["Add"]),
    el("label", { className: "field-label" }, [
      "Provider",
      el("select", { name: "provider" }, providers.map((p) => el("option", { value: p }, [p]))),
    ]),
    el("label", { className: "field-label" }, [
      "Name",
      el("input", { type: "text", name: "secret_name", className: "name", autocomplete: "off" }),
    ]),
    el("div", { "data-wizard-step": "fields" }),
    el("button", { type: "button", className: "primary", "data-action": "wizard-save", onClick: saveWizard }, ["Save"]),
    el("button", { type: "button", className: "ghost", "data-action": "wizard-cancel", onClick: () => { state.wizard = false; paint(); } }, ["Cancel"]),
  ]);
}

function registerPage() {
  const layout = layoutMode(state.width);
  const selected = state.items.find((i) => i.name === state.selected) || null;
  const list = el("div", { className: "list", "data-pane": "list" }, [
    el("ul", null, state.items.map((item) =>
      el("li", { "data-name": item.name }, [
        el("button", { type: "button", className: "row name", "data-action": "select", "data-name": item.name, onClick: () => { state.selected = item.name; paint(); } }, [item.name]),
      ])
    )),
  ]);
  const kids = [
    el("header", { className: "toolbar" }, [
      el("h1", null, ["Register"]),
      el("button", { type: "button", className: "primary", "data-action": "add", onClick: () => { state.wizard = true; paint(); } }, ["Add"]),
    ]),
    errNode(),
    el("div", { className: "workspace" }, layout === "list-inspector" ? [list, inspector(selected)] : [list]),
  ];
  if (layout === "list-only" && selected) kids.push(sheet(selected));
  if (state.wizard) kids.push(wizard());
  return el("section", { "data-page": "register", "data-layout": layout }, kids.filter(Boolean));
}

function accountPage() {
  const removeOk = !(state.passkeys.length <= 1 && !(state.session && state.session.has_password));
  return el("section", { "data-page": "account" }, [
    el("h1", null, ["Account"]),
    el("p", { className: "muted" }, [state.session ? state.session.email : ""]),
    errNode(),
    el("h2", null, ["Sessions"]),
    el("table", { "data-list": "sessions" }, [
      el("thead", null, [el("tr", null, ["Label", "Kind", "Created", "Last seen", ""].map((h) => el("th", null, [h])))]),
      el("tbody", null, state.sessions.map((s) =>
        el("tr", { "data-session-id": s.id, "data-current": s.current ? "true" : null }, [
          el("td", { className: "name" }, [s.label]),
          el("td", null, [s.kind]),
          el("td", null, [s.created]),
          el("td", null, [s.last_seen]),
          el("td", null, [el("button", { type: "button", className: "danger", "data-action": "revoke", "data-session-id": s.id, onClick: () => revokeSession(s.id) }, ["Revoke"])]),
        ])
      )),
    ]),
    el("h2", null, ["Passkeys"]),
    el("ul", { "data-list": "passkeys" }, state.passkeys.map((p) =>
      el("li", { "data-passkey-id": p.id }, [
        el("span", { className: "name" }, [p.id]),
        el("span", { className: "muted" }, [p.created]),
        el("button", { type: "button", className: "danger", "data-action": "remove", "data-passkey-id": p.id, disabled: !removeOk, onClick: () => { if (removeOk) removePasskey(p.id); } }, ["Remove"]),
      ])
    )),
    el("button", { type: "button", className: "primary", "data-action": "add-passkey", onClick: addPasskey }, ["Add passkey"]),
  ]);
}

function activityPage() {
  return el("section", { "data-page": "activity" }, [
    el("h1", null, ["Activity"]),
    el("ul", { "data-list": "audit" }, state.events.map((e) =>
      el("li", null, [
        el("span", null, [e.action || ""]),
        el("span", { className: "name" }, [e.name || ""]),
        el("span", { className: "muted" }, [e.at || ""]),
      ])
    )),
  ]);
}

function paint() {
  const root = document.body;
  const g = resolveGate();
  let node;
  if (!state.session) {
    node = gatePage(g);
  } else if (state.screen === "device" || (state.screen === "boot" && g.showApprove)) {
    node = chrome("device", devicePage());
  } else if (state.screen === "account") {
    node = chrome("account", accountPage());
  } else if (state.screen === "activity") {
    node = chrome("activity", activityPage());
  } else {
    node = chrome("register", registerPage());
  }
  root.replaceChildren(node);
}

async function go(screen) {
  state.screen = screen;
  state.error = "";
  if (screen === "account") await loadAccount();
  if (screen === "activity") await loadActivity();
  if (screen === "register") await loadVault();
  paint();
}

async function loadAccount() {
  const s = await req("GET", "/api/v1/sessions");
  state.sessions = (s.data && s.data.sessions) || [];
  const p = await req("GET", "/api/auth/passkeys");
  state.passkeys = (p.data && p.data.passkeys) || [];
}

async function loadActivity() {
  const a = await req("GET", "/api/v1/audit");
  const ev = (a.data && (a.data.events || a.data.audit)) || [];
  state.events = ev.map((e) => ({
    action: e.action || e.event || "",
    name: e.name || e.id || "",
    at: e.at || e.created || "",
  }));
}

async function loadVault() {
  const v = await req("GET", "/api/v1/vault");
  const entries = (v.data && v.data.entries) || [];
  const groups = {};
  for (const e of entries) {
    const name = e.name || "";
    if (!groups[name]) groups[name] = { name, fields: [] };
    groups[name].fields.push({ key: name.split("/").pop() || name, secret: true, value: "" });
  }
  state.items = Object.values(groups);
}

async function copyField(key) {
  const item = state.items.find((i) => i.name === state.selected);
  const f = item && item.fields.find((x) => x.key === key);
  const text = (f && f.value) || "";
  if (navigator.clipboard && navigator.clipboard.writeText) await navigator.clipboard.writeText(text);
}

function showField(key, on) {
  const n = document.querySelector('[data-value="' + CSS.escape(key) + '"]');
  if (!n) return;
  const item = state.items.find((i) => i.name === state.selected);
  const f = item && item.fields.find((x) => x.key === key);
  n.hidden = !on;
  n.textContent = on && f ? f.value : "";
}

function emailFromDom() {
  const n = document.querySelector('input[name="email"]');
  if (n && n.value) return n.value.trim().toLowerCase();
  return (state.remember && state.remember.email) || "";
}

function passwordFromDom() {
  const n = document.querySelector('input[name="password"]');
  return n ? n.value : "";
}

async function afterAuth(data, email, hasPasskey) {
  saveRemember(email, hasPasskey);
  state.remember = loadRemember();
  const ses = await req("GET", "/api/session");
  if (ses.status === 200) {
    state.session = ses.data;
    state.screen = state.userCode ? "device" : "register";
    await loadVault();
    paint();
    return;
  }
  state.error = (data && data.error) || FAIL;
  paint();
}

async function onPassword() {
  const email = emailFromDom();
  const password = passwordFromDom();
  if (!state.method && email) {
    const st = await req("POST", "/api/auth/start", { email });
    if (st.status === 200) state.method = st.data.method;
  }
  const url = state.method === "register" ? "/api/auth/password/register" : "/api/auth/password/login";
  const res = await req("POST", url, { email, password });
  if (res.status === 200) {
    await afterAuth(res.data, email, false);
    return;
  }
  state.error = (res.data && res.data.error) || FAIL;
  paint();
}

async function onPasskey() {
  const email = emailFromDom();
  try {
    if (!state.session && !state.method && email) {
      const st = await req("POST", "/api/auth/start", { email });
      if (st.status === 200) state.method = st.data.method;
    }
    let res;
    if (state.method === "register" || (state.session && state.screen === "account")) {
      res = await passkeyCreate(email || (state.session && state.session.email) || "");
    } else {
      res = await passkeyGet(email || undefined, false);
    }
    if (res.status === 200) {
      await afterAuth(res.data, email || (state.session && state.session.email) || "", true);
      return;
    }
    state.error = (res.data && res.data.error) || FAIL;
  } catch {
    state.error = FAIL;
  }
  paint();
}

async function startConditional() {
  const g = resolveGate();
  if (!g.showEmail || !window.PublicKeyCredential) return;
  try {
    if (PublicKeyCredential.isConditionalMediationAvailable) {
      const ok = await PublicKeyCredential.isConditionalMediationAvailable();
      if (!ok) return;
    }
    const res = await passkeyGet(undefined, true);
    if (res.status === 200) {
      await afterAuth(res.data, "", true);
    }
  } catch { /* ignored: user typed email instead */ }
}

async function approve(code) {
  const sealed = { alg: "x25519-xchacha20poly1305" };
  const res = await req("POST", "/api/v1/device/approve", { user_code: code, sealed_dek: sealed });
  if (res.status === 200) {
    state.screen = "register";
    await loadVault();
    paint();
    return;
  }
  state.error = (res.data && res.data.error) || FAIL;
  paint();
}

async function revokeSession(id) {
  const res = await req("DELETE", "/api/v1/sessions/" + encodeURIComponent(id));
  if (res.status === 200) {
    const cur = state.sessions.find((s) => s.id === id);
    if (cur && cur.current) {
      clearRemember();
      state.session = null;
      state.screen = "gate";
      paint();
      return;
    }
    await loadAccount();
    paint();
    return;
  }
  state.error = (res.data && res.data.error) || FAIL;
  paint();
}

async function removePasskey(id) {
  const res = await req("DELETE", "/api/auth/passkeys/" + encodeURIComponent(id));
  if (res.status === 200) {
    await loadAccount();
    paint();
    return;
  }
  state.error = (res.data && res.data.error) || FAIL;
  paint();
}

async function addPasskey() {
  if (!state.session) return;
  try {
    const res = await passkeyCreate(state.session.email);
    if (res.status === 200) {
      await loadAccount();
      paint();
      return;
    }
    state.error = (res.data && res.data.error) || FAIL;
  } catch {
    state.error = FAIL;
  }
  paint();
}

async function saveWizard() {
  const name = (document.querySelector('input[name="secret_name"]') || {}).value || "";
  const provider = (document.querySelector("select[name=provider]") || {}).value || "";
  if (!name) return;
  const get = await req("GET", "/api/v1/vault");
  const entries = (get.data && get.data.entries) || [];
  entries.push({ name, ciphertext: "", meta: { provider } });
  await req("PUT", "/api/v1/vault", { entries });
  state.wizard = false;
  await loadVault();
  paint();
}

async function boot() {
  const q = qs();
  state.userCode = q.user_code;
  state.ephPub = q.eph_pub;
  state.remember = loadRemember();
  state.width = window.innerWidth;
  const ses = await req("GET", "/api/session");
  if (ses.status === 200) {
    state.session = ses.data;
    state.screen = "device";
    paint();
    return;
  }
  state.session = null;
  state.screen = "gate";
  paint();
  const g = resolveGate();
  if (g.kind === "remembered-passkey") {
    onPasskey();
  } else if (g.showEmail) {
    startConditional();
  }
}

window.addEventListener("resize", () => {
  const w = window.innerWidth;
  if ((w >= BREAKPOINT) !== (state.width >= BREAKPOINT)) {
    state.width = w;
    paint();
  } else {
    state.width = w;
  }
});

boot();
