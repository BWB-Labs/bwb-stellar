import json
import re
import sys
import urllib.request
from pathlib import Path

SUBMISSION = "https://communityfund.stellar.org/submissions/rec8bhUAafIcSJUzA"
ROOT = Path(__file__).resolve().parent.parent
SCF = ROOT / "docs/scf"


def submission_text():
    request = urllib.request.Request(SUBMISSION, headers={"User-Agent": "Mozilla/5.0"})
    html = urllib.request.urlopen(request, timeout=30).read().decode()
    chunks = re.findall(r'self\.__next_f\.push\(\[1,("(?:[^"\\]|\\.)*")\]\)', html)
    return "".join(json.loads(chunk) for chunk in chunks)


def criteria_by_deliverable(text):
    start = text.index("### Deliverable 3 — ")
    end = text.index("### Deliverable 8 — ")
    parts = re.split(r"(?:\*\*)?### Deliverable (\d) — ", text[start:end])[1:]
    return {
        number: re.findall(r"\*\*\d\. (.+?)\*\*", body.split("**How to Measure Completion:**", 1)[1])
        for number, body in zip(parts[::2], parts[1::2])
    }


def failures_of(number, criteria):
    page = SCF / f"tranche-2-d{number}.md"
    index = SCF / f"evidence/d{number}/index.json"
    if not page.exists() or not index.exists():
        return [f"D{number}: page or index missing"]
    text = page.read_text()
    entries = json.loads(index.read_text())["criteria"]
    headings = re.findall(r"^## Criterion (\d) — (\w+)", text, re.M)
    failures = []
    if len(criteria) != 4:
        failures.append(f"D{number}: the submission lists {len(criteria)} criteria")
    if re.findall(r"^> (.+)$", text, re.M) != criteria:
        failures.append(f"D{number}: the page's quotes differ from the submission")
    if [entry["text"] for entry in entries] != criteria:
        failures.append(f"D{number}: the index's criteria differ from the submission")
    if [(str(entry["criterion"]), entry["status"]) for entry in entries] != headings:
        failures.append(f"D{number}: the page's headings and the index's statuses disagree")
    for entry in entries:
        if entry["status"] != "open" and not entry["evidence"]:
            failures.append(f"D{number} criterion {entry['criterion']}: {entry['status']} with no evidence")
        if entry["status"] != "open" and "CI" in entry["text"] and entry["ci_run"] is None:
            failures.append(f"D{number} criterion {entry['criterion']}: {entry['status']} with no CI run")
    return failures


failures = [f for number, criteria in criteria_by_deliverable(submission_text()).items() for f in failures_of(number, criteria)]
print("\n".join(failures) or "D3 to D7: every criterion matches the submission word for word")
sys.exit(1 if failures else 0)
