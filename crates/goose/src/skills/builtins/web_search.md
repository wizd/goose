---
name: web-search
description: Search the web via a China-reachable SearXNG instance and extract page content with r.vcorp.ai. Use whenever the task needs current information, facts not in training data, or content from a specific URL.
---

SearXNG default engines (google, duckduckgo, brave, startpage, wikipedia) are blocked or CAPTCHA'd from mainland China. Every search MUST pass an explicit `engines=` list from the groups below. Do not invent or fall back to other engines.

## Search

```bash
SEARXNG_BASE="${SEARXNG_URL:-https://searxng-g44cookggsos08gkgg0cogc0.vcorp.ai}"
```

Pick one engine group by task type. Do not omit `engines=`.

**General** — news, facts, current events (`quark,yandex,encyclosearch,wiby`):
```bash
curl -sG --max-time 30 \
  --data-urlencode "q=your query here" \
  --data-urlencode "engines=quark,yandex,encyclosearch,wiby" \
  --data "format=json" "${SEARXNG_BASE}/search" | python3 -c "
import json, sys
d = json.load(sys.stdin)
for r in d.get('results', [])[:8]:
    print(r['url'])
    print(r.get('content',''))
    print()
if not d.get('results'):
    print('NO RESULTS; unresponsive:', d.get('unresponsive_engines'))
"
```

**Academic** — papers, citations, scholarly topics (`google scholar,semantic scholar,openalex,crossref,pubmed`):
```bash
curl -sG --max-time 30 \
  --data-urlencode "q=your query here" \
  --data-urlencode "engines=google scholar,semantic scholar,openalex,crossref,pubmed" \
  --data "format=json" "${SEARXNG_BASE}/search" | python3 -c "
import json, sys
d = json.load(sys.stdin)
for r in d.get('results', [])[:8]:
    print(r['url'])
    print(r.get('content',''))
    print()
if not d.get('results'):
    print('NO RESULTS; unresponsive:', d.get('unresponsive_engines'))
"
```

**Developer** — code, docs, packages (`github,hackernews,stackoverflow,crates.io,npm,pkg.go.dev,mdn,huggingface`):
```bash
curl -sG --max-time 30 \
  --data-urlencode "q=your query here" \
  --data-urlencode "engines=github,hackernews,stackoverflow,crates.io,npm,pkg.go.dev,mdn,huggingface" \
  --data "format=json" "${SEARXNG_BASE}/search" | python3 -c "
import json, sys
d = json.load(sys.stdin)
for r in d.get('results', [])[:8]:
    print(r['url'])
    print(r.get('content',''))
    print()
if not d.get('results'):
    print('NO RESULTS; unresponsive:', d.get('unresponsive_engines'))
"
```

If a group returns no results, retry with another listed group. Do not add engines outside these lists.

**Forbidden engines** (blocked, CAPTCHA'd, or timed out from this instance):
`google`, `duckduckgo`, `brave`, `startpage`, `bing`, `baidu`, `sogou`, `360search`, `mojeek`, `qwant`, `yahoo`, `arxiv`, `yacy`, `seznam`, `ask`, `mwmbl`.
`google scholar` is allowed; `google` is not.

**Optional fallback — Tavily** (requires `TAVILY_API_KEY`; use only if every SearXNG group fails):
```bash
python3 -c "
import os, json, urllib.request
req = urllib.request.Request(
    'https://api.tavily.com/search',
    data=json.dumps({'api_key': os.environ['TAVILY_API_KEY'], 'query': 'your query here', 'max_results': 5}).encode(),
    headers={'Content-Type': 'application/json'},
)
with urllib.request.urlopen(req, timeout=30) as resp:
    r = json.load(resp)
for res in r.get('results', []):
    print(res['url'])
    print(res.get('content', ''))
    print()
"
```

## Extract page content

Always read result URLs through r.vcorp.ai. Do not curl the target URL directly.

```bash
url="https://example.com"
tmpfile=$(mktemp /tmp/page-XXXXXX)
if [ -n "${JINA_API_KEY:-}" ]; then
  curl -sL --max-time 30 -H "Authorization: Bearer $JINA_API_KEY" "https://r.vcorp.ai/${url}" > "$tmpfile"
else
  curl -sL --max-time 30 "https://r.vcorp.ai/${url}" > "$tmpfile"
fi
wc -c "$tmpfile"
head -c 15000 "$tmpfile"
```

If the page is larger than 15 000 characters, show both head and tail so the user can decide whether to read the full file:
```bash
echo "--- HEAD ---"
head -c 7500 "$tmpfile"
echo ""
echo "--- TAIL ---"
tail -c 7500 "$tmpfile"
echo ""
echo "(Full content saved to $tmpfile)"
```

## Rules

- Always quote search queries to avoid shell word-splitting.
- Always pass an explicit `engines=` list from the groups above.
- Respect robots.txt for scraping; do not hammer a host with repeated requests.
- Never send authentication cookies or session tokens to external URLs.
- If a page returns a login wall or CAPTCHA, report the URL and stop; do not attempt to bypass it.
