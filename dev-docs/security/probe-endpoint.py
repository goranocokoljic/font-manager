"""Live checks of the running app's local API: browser-style requests must be refused."""
import json, http.client, urllib.parse
ep = json.load(open(r"C:\Users\Micha\AppData\Local\FontManager\data\endpoint.json"))
u = urllib.parse.urlparse(ep["url"]); port = u.port
def req(label, method, path, headers=None, body=None, host=None):
    c = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    h = {"Host": host or f"127.0.0.1:{port}", "Content-Type": "application/json"}
    h.update(headers or {})
    c.putrequest(method, path, skip_host=True, skip_accept_encoding=True)
    for k, v in h.items(): c.putheader(k, v)
    data = (body or "").encode(); c.putheader("Content-Length", str(len(data))); c.endheaders(data)
    r = c.getresponse(); text = r.read().decode(errors="replace").replace(ep["token"], "<TOKEN>")
    print(f"{label:52} -> {r.status} {text[:110]}")
    c.close()
auth = {"Authorization": f"Bearer {ep['token']}"}
req("GET /v1/version, no token", "GET", "/v1/version")
req("POST list_sources, no token", "POST", "/v1/list_sources", body="{}")
req("POST list_sources, wrong token", "POST", "/v1/list_sources", {"Authorization": "Bearer " + "0"*64}, "{}")
req("POST list_sources, token + Origin header", "POST", "/v1/list_sources", {**auth, "Origin": "https://evil.example"}, "{}")
req("POST list_sources, token, Host evil.example", "POST", "/v1/list_sources", auth, "{}", host=f"evil.example:{port}")
req("POST list_sources, token, Host 127.0.0.1 other port", "POST", "/v1/list_sources", auth, "{}", host="127.0.0.1:1")
req("POST list_sources, token (good)", "POST", "/v1/list_sources", auth, "{}")
req("POST /mcp, no token", "POST", "/mcp", body='{"jsonrpc":"2.0","id":1,"method":"ping"}')
req("POST answer_mcp_confirmation id 999999, token", "POST", "/v1/answer_mcp_confirmation", auth, '{"id":999999,"allow":true}')
req("POST set_mcp_grant tag_fonts=false, token", "POST", "/v1/set_mcp_grant", auth, '{"tool":"tag_fonts","granted":false}')
req("POST regenerate_endpoint_token route exists? (GET)", "GET", "/v1/regenerate_endpoint_token", auth)
