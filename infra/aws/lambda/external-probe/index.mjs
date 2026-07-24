import { SSMClient, GetParameterCommand } from "@aws-sdk/client-ssm";

const ssm = new SSMClient({});
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function probeTarget(url) {
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(10_000) });
      if (res.ok) {
        console.log(`attempt ${attempt}: HTTP ${res.status} ok`);
        return true;
      }
      console.warn(`attempt ${attempt}: HTTP ${res.status}`);
    } catch (err) {
      console.error(`attempt ${attempt}: ${err.name}`);
    }
    if (attempt < 3) await sleep(30_000);
  }
  return false;
}

async function getToken() {
  const out = await ssm.send(
    new GetParameterCommand({
      Name: process.env.PAT_PARAM_NAME,
      WithDecryption: true,
    }),
  );
  return out.Parameter.Value;
}

async function gh(token, method, path, body) {
  const res = await fetch(`https://api.github.com${path}`, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: "application/vnd.github+json",
      "User-Agent": "bons8i-external-probe",
      ...(body ? { "Content-Type": "application/json" } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) {
    throw new Error(`GitHub API ${method} ${path}: HTTP ${res.status}`);
  }
  return res.json();
}

export async function handler(event) {
  const url = event?.url ?? process.env.TARGET_URL;
  const repo = process.env.GITHUB_REPO;

  const ok = await probeTarget(url);
  const token = await getToken();

  const openIssues = await gh(
    token,
    "GET",
    `/repos/${repo}/issues?labels=outage&state=open&per_page=1`,
  );
  const openIssue = openIssues[0];

  if (!ok && !openIssue) {
    const issue = await gh(token, "POST", `/repos/${repo}/issues`, {
      title: "[OUTAGE] bons8i.hagaspa.com is unreachable",
      labels: ["alert", "outage"],
      body: "External probe failed 3 consecutive attempts (10s timeout, 30s interval).",
    });
    console.log(`opened issue #${issue.number}`);
  } else if (ok && openIssue) {
    await gh(token, "POST", `/repos/${repo}/issues/${openIssue.number}/comments`, {
      body: "External probe succeeded. Service recovered.",
    });
    await gh(token, "PATCH", `/repos/${repo}/issues/${openIssue.number}`, {
      state: "closed",
    });
    console.log(`closed issue #${openIssue.number}`);
  }

  return { ok, openIssue: openIssue?.number ?? null };
}
