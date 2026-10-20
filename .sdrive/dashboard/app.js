async function fetchJson(url) {
  try {
    const res = await fetch(url);
    return await res.json();
  } catch (error) {
    return null;
  }
}

function escapeHtml(value) {
  return String(value == null ? "" : value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function gateClass(status) {
  if (status === "approved") return "gate approved";
  if (status === "in_progress") return "gate in_progress";
  return "gate pending";
}

/* ============================================================
   PAGINATION
   A tiny shared pager. Each paginated view keeps its own page
   number in `pagerState` and re-renders only its own list, so
   switching pages never refetches from the server.

   Usage:
     const slice = paginate(items, "skills", 9);
     ...render slice.items...
     container.appendChild(renderPager(items.length, "skills", 9, renderSkills));
   ============================================================ */
const pagerState = {};

// Clamp the stored page for `key` and return the visible slice.
function paginate(items, key, perPage) {
  const total = items.length;
  const pages = Math.max(1, Math.ceil(total / perPage));
  let page = pagerState[key] || 1;
  if (page > pages) page = pages;
  if (page < 1) page = 1;
  pagerState[key] = page;

  const start = (page - 1) * perPage;
  return {
    items: items.slice(start, start + perPage),
    page,
    pages,
    total,
    start,
    end: Math.min(start + perPage, total)
  };
}

// Build the page-number buttons, collapsing long runs with an ellipsis.
// Always shows first, last, current, and one neighbour on each side.
function pageNumbers(page, pages) {
  if (pages <= 7) {
    return Array.from({ length: pages }, (_, i) => i + 1);
  }
  const out = [1];
  const from = Math.max(2, page - 1);
  const to = Math.min(pages - 1, page + 1);
  if (from > 2) out.push("…");
  for (let i = from; i <= to; i++) out.push(i);
  if (to < pages - 1) out.push("…");
  out.push(pages);
  return out;
}

// Render the pager bar. Returns null when everything fits on one page so
// callers can append unconditionally.
function renderPager(total, key, perPage, onNavigate) {
  const pages = Math.max(1, Math.ceil(total / perPage));
  if (pages <= 1) return null;

  const page = Math.min(pagerState[key] || 1, pages);
  const start = (page - 1) * perPage + 1;
  const end = Math.min(page * perPage, total);

  const wrap = document.createElement("div");
  wrap.className = "pager";

  const summary = document.createElement("div");
  summary.className = "pager-summary";
  summary.innerHTML = `Showing <strong>${start}–${end}</strong> of <strong>${total}</strong>`;
  wrap.appendChild(summary);

  const controls = document.createElement("div");
  controls.className = "pager-controls";

  const goTo = (next) => {
    if (next < 1 || next > pages || next === page) return;
    pagerState[key] = next;
    onNavigate();
  };

  const makeBtn = (label, target, opts = {}) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "pager-btn" + (opts.active ? " is-active" : "");
    btn.textContent = label;
    if (opts.title) btn.title = opts.title;
    if (opts.ariaLabel) btn.setAttribute("aria-label", opts.ariaLabel);
    if (opts.active) btn.setAttribute("aria-current", "page");
    btn.disabled = !!opts.disabled;
    if (!opts.disabled && !opts.active) {
      btn.addEventListener("click", () => goTo(target));
    }
    return btn;
  };

  controls.appendChild(
    makeBtn("‹", page - 1, { disabled: page === 1, ariaLabel: "Previous page", title: "Previous page" })
  );

  pageNumbers(page, pages).forEach((n) => {
    if (n === "…") {
      const gap = document.createElement("span");
      gap.className = "pager-ellipsis";
      gap.textContent = "…";
      controls.appendChild(gap);
    } else {
      controls.appendChild(makeBtn(String(n), n, { active: n === page, ariaLabel: `Page ${n}` }));
    }
  });

  controls.appendChild(
    makeBtn("›", page + 1, { disabled: page === pages, ariaLabel: "Next page", title: "Next page" })
  );

  wrap.appendChild(controls);
  return wrap;
}

// Human-readable gate metadata. Gates are approval checkpoints a feature must
// clear before it can advance; without a label users mistake them for metrics.
const GATE_DEFS = [
  { key: "specification", label: "Spec", hint: "Specification approved" },
  { key: "planning", label: "Plan", hint: "Plan approved" },
  { key: "implementation", label: "Impl", hint: "Implementation approved" },
  { key: "review", label: "Review", hint: "Review approved" }
];

function gateStatusMeta(status) {
  if (status === "approved") return { icon: "✓", text: "approved" };
  if (status === "in_progress") return { icon: "●", text: "in progress" };
  return { icon: "○", text: "pending" };
}

function phaseTone(phase) {
  if (phase === "completed") return "green";
  if (phase === "ongoing") return "blue";
  if (phase === "blocked") return "red";
  return "amber";
}

function verificationTone(status) {
  if (status === "pass") return "green";
  if (status === "warn") return "amber";
  if (status === "fail") return "red";
  return "slate";
}

function initials(name) {
  return String(name || "?")
    .trim()
    .split(/[\s-_]+/)
    .map((w) => w[0])
    .slice(0, 2)
    .join("")
    .toUpperCase();
}

function formatDate(iso) {
  if (!iso) return "—";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric"
  });
}

function formatDateTime(iso) {
  if (!iso) return "—";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit"
  });
}

function getFilterValues() {
  return {
    search: document.getElementById("filter-search").value.trim().toLowerCase(),
    phase: document.getElementById("filter-phase").value,
    verification: document.getElementById("filter-verification").value,
    tests: document.getElementById("filter-tests").value,
    currentVersionOnly: document.getElementById("filter-current-version").checked
  };
}

function featureMatchesFilters(feature, version, filters, verification, tests) {
  if (filters.search && !feature.name.toLowerCase().includes(filters.search)) {
    return false;
  }

  if (filters.phase && version.phase !== filters.phase) {
    return false;
  }

  if (filters.verification) {
    const status =
      verification.status === "not_run" || !verification.status
        ? "not_run"
        : verification.status;

    if (status !== filters.verification) {
      return false;
    }
  }

  if (filters.tests) {
    const testStatus =
      tests.status === "not_run" || !tests.status ? "not_run" : tests.status;

    if (testStatus !== filters.tests) {
      return false;
    }
  }

  return true;
}

/* ============================================================
   PROJECT INFO
   ============================================================ */
function renderProjectInfo(project) {
  const name = (project && project.name) || "project";

  const sidebar = document.getElementById("sidebar-project");
  if (sidebar) sidebar.textContent = name;

  if (project && project.name) {
    document.title = `${project.name} · SpecDrive Dashboard`;
  }
}

/* ============================================================
   PROJECT SWITCHER
   ============================================================ */
async function renderProjectSwitcher() {
  const select = document.getElementById("project-switcher");
  if (!select) return;

  let data;
  try {
    data = await fetchJson("/api/projects");
  } catch (error) {
    return;
  }

  const projects = (data && data.projects) || [];
  if (!projects.length) return;

  select.innerHTML = projects
    .map(
      (project) =>
        `<option value="${escapeHtml(project.path)}"${project.active ? " selected" : ""}>${escapeHtml(project.label)}</option>`
    )
    .join("");

  select.title = "Switch the project this dashboard is showing";
  select.disabled = projects.length < 2;
  select.dataset.bound = "true";
}

function setupProjectSwitcher() {
  const select = document.getElementById("project-switcher");
  if (!select) return;

  select.addEventListener("change", async () => {
    const target = select.value;
    select.disabled = true;

    try {
      const response = await fetch("/api/switch-project", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ path: target })
      });

      if (!response.ok) {
        const data = await response.json().catch(() => ({}));
        throw new Error(data.error || "Could not switch project");
      }

      window.location.reload();
    } catch (error) {
      select.disabled = false;
      alert(error.message);
    }
  });
}

/* ============================================================
   GIT INFO
   ============================================================ */
function renderGitInfo(git) {
  const sidebar = document.getElementById("sidebar-git");
  const chip = document.getElementById("topbar-git");

  renderBranchingModel(git && git.branching);

  if (!git || !git.available) {
    if (sidebar) sidebar.textContent = "Not a git repository";
    if (chip) chip.textContent = "No git";
    return;
  }

  if (sidebar) {
    sidebar.innerHTML = `
      <div><span class="git-branch-name">${escapeHtml(git.branch)}</span></div>
      <div class="git-meta">${escapeHtml(git.commit)} · ${escapeHtml(git.author || "")}</div>
      ${git.mergedFrom ? `<div class="git-meta">merged from ${escapeHtml(git.mergedFrom)}</div>` : ""}
    `;
  }

  if (chip) {
    const sync =
      git.ahead || git.behind
        ? ` · ↑${git.ahead} ↓${git.behind}`
        : "";
    chip.innerHTML = `
      <span class="dot ${git.dirty === true ? "dirty" : git.dirty === null ? "unknown" : ""}"></span>
      <span class="branch">${escapeHtml(git.branch)}</span>
      <span>${escapeHtml(git.commit)}${sync}</span>
    `;
    chip.title = `${git.subject || ""}${git.mergedFrom ? `\nMerged from: ${git.mergedFrom}` : ""}`;
  }
}

/* ============================================================
   BRANCHING MODEL
   ============================================================ */
// Read-out only. The integration branch is resolved from the repository and each
// feature's landing target comes from the `base` recorded on it. Nothing here is a
// setting, so this display cannot change how a feature is archived - it describes
// the setup the project already has.
function renderBranchingModel(branching) {
  const el = document.getElementById("sidebar-branching");
  if (!el) return;

  if (!branching || !branching.label) {
    el.innerHTML = "";
    el.removeAttribute("title");
    return;
  }

  // The concrete branch is the fact; the model name is the hint. When features land
  // on more than one branch there is no single integration branch to show.
  const target = branching.integration.length
    ? branching.integration.join(", ")
    : branching.defaultBranch || "no default branch";

  const stack = branching.stackedCount && branching.label !== "Stacked features"
    ? `<div class="branching-stack">${branching.stackedCount} stacked on a peer branch</div>`
    : "";

  el.innerHTML = `
    <div class="branching-model">${escapeHtml(branching.label)}</div>
    <div class="branching-targets">lands on ${escapeHtml(target)}</div>
    ${stack}
  `;

  el.title = branching.defaultBranchSource
    ? `${branching.detail}\n\nDefault branch resolved from ${branching.defaultBranchSource}.`
    : branching.detail;
}

/* ============================================================
   OVERVIEW
   ============================================================ */
function renderOverview(onboarding, git, product, productType, health) {
  const wrap = document.createElement("div");
  wrap.className = "overview-page";

  const summary = product?.summary || null;

  wrap.innerHTML = `
    ${renderOverviewHero(summary, git, productType)}
    ${renderHealthCard(health)}
    ${renderOverviewKpis(onboarding, git, summary)}
    ${renderOverviewPanels(onboarding, git, summary)}
  `;

  return wrap;
}

/* Project health. The number is deliberately inseparable from its breakdown:
   every component opens to show the rows behind it, and "do this next" is the only
   call to action on the page. A score without those two things is a vibe, not a
   measurement. */
function renderHealthCard(health) {
  if (!health) return "";

  if (!health.available) {
    return `
    <section class="ov-panel health-card">
      <header class="ov-panel-head">
        <span class="ov-panel-icon">+</span>
        <h3>Project Health</h3>
      </header>
      <div class="ov-panel-body">
        <div class="ov-empty">${escapeHtml(health.reason || "Nothing to score yet.")}</div>
      </div>
    </section>
    `;
  }

  const tone = health.score >= 80 ? "green" : health.score >= 55 ? "amber" : "red";

  const components = health.components
    .map((component) => {
      const pct = component.percent === null ? "not scored" : `${component.percent}%`;
      const width = component.percent === null ? 0 : component.percent;
      const evidence = (component.evidence || [])
        .map((line) => `<li>${escapeHtml(line)}</li>`)
        .join("");
      const more = component.evidenceMore
        ? `<li class="muted">and ${component.evidenceMore} more</li>`
        : "";
      return `
        <details class="health-row">
          <summary>
            <span class="health-row-label">${escapeHtml(component.label)}</span>
            <span class="health-row-bar"><span style="width:${width}%"></span></span>
            <span class="health-row-pct">${pct}</span>
          </summary>
          <div class="health-row-body">
            <p>${escapeHtml(component.detail)}</p>
            ${evidence || more ? `<ul class="health-evidence">${evidence}${more}</ul>` : ""}
          </div>
        </details>
      `;
    })
    .join("");

  const next = health.nextAction
    ? `
      <div class="health-next">
        <div class="health-next-label">Do this next</div>
        <div class="health-next-title">${escapeHtml(health.nextAction.title)}</div>
        <div class="health-next-command"><code>${escapeHtml(health.nextAction.command)}</code>${
          health.nextAction.points > 0
            ? `<span class="health-next-gain">+${health.nextAction.points}</span>`
            : ""
        }</div>
        <div class="health-next-reason">${escapeHtml(health.nextAction.reason)}</div>
      </div>
    `
    : "";

  const blockers = health.blockers.length
    ? `
      <div class="health-blockers">
        <div class="health-blockers-label">${health.blockers.length} live version(s) cannot be archived</div>
        <ul class="health-evidence">
          ${health.blockers
            .map((entry) =>
              (entry.blockers || [])
                .map(
                  (blocker, index) =>
                    `<li>${
                      index === 0 ? `<strong>${escapeHtml(entry.feature)}</strong> - ` : ""
                    }${escapeHtml(blocker)}</li>`
                )
                .join("")
            )
            .join("")}
        </ul>
      </div>
    `
    : "";

  return `
    <section class="ov-panel health-card">
      <header class="ov-panel-head">
        <span class="ov-panel-icon">+</span>
        <h3>Project Health</h3>
        <span class="chip ${tone}">${escapeHtml(health.band)}</span>
      </header>
      <div class="ov-panel-body health-body">
        <div class="health-score">
          <div class="health-score-value">${health.score}</div>
          <div class="health-score-meta">
            <div>out of 100</div>
            <div class="muted">${health.liveCount} live feature(s) scored</div>
          </div>
        </div>
        <div class="health-components">${components}</div>
        ${next}
        ${blockers}
      </div>
    </section>
  `;
}

/* Hero banner — the product's identity at a glance. */
function renderOverviewHero(summary, git, productType) {
  const title = summary?.title || "Project Overview";
  const vision = summary?.vision || "";
  const status = summary?.status || "";
  const branch = git?.branch || "—";

  const pt = productType || null;
  const typeBadge = pt
    ? `
        <span class="ov-type-badge" data-type="${escapeHtml(pt.type)}" title="${escapeHtml(pt.description || "")}">
          <span class="ov-type-icon">${escapeHtml(pt.icon || "◆")}</span>
          <span class="ov-type-label">${escapeHtml(pt.label)}</span>
          <span class="ov-type-tagline">${escapeHtml(pt.tagline || "")}</span>
        </span>`
    : "";

  return `
    <div class="ov-hero">
      <div class="ov-hero-glow"></div>
      <div class="ov-hero-body">
        <div class="ov-hero-eyebrow">
          <span class="ov-hero-dot"></span>
          <span>Product Overview</span>
          ${status ? `<span class="ov-hero-status">${escapeHtml(status)}</span>` : ""}
          ${typeBadge}
        </div>
        <h2 class="ov-hero-title">${escapeHtml(title)}</h2>
        ${vision ? `<p class="ov-hero-vision">${escapeHtml(vision)}</p>` : ""}
        <div class="ov-hero-meta">
          <span class="ov-hero-meta-item"><span class="ov-hero-meta-icon">⎇</span>${escapeHtml(branch)}</span>
          ${git?.commit ? `<span class="ov-hero-meta-item"><span class="ov-hero-meta-icon">◈</span>${escapeHtml(git.commit)}</span>` : ""}
          ${git?.mergedFrom ? `<span class="ov-hero-meta-item"><span class="ov-hero-meta-icon">⇄</span>merged from ${escapeHtml(git.mergedFrom)}</span>` : ""}
        </div>
      </div>
    </div>
  `;
}

/* KPI strip — the headline numbers, presented as a single unified band. */
// A drafted brief is NOT an approved plan. The onboarding scan writes a product
// brief derived from the README, so the page must never report that as approved.
function isDraftPlan(summary) {
  return /draft/i.test(summary?.status || "");
}

function productStatusSub(summary) {
  if (!summary?.title) return "no product plan";
  return isDraftPlan(summary) ? "draft, not confirmed" : "product plan approved";
}

function productStatusTone(summary) {
  if (!summary?.status) return "slate";
  return isDraftPlan(summary) ? "slate" : "green";
}

function renderOverviewKpis(onboarding, git, summary) {
  const goals = summary?.goals || [];
  const users = summary?.targetUsers || [];
  const arch = summary?.architecture || [];
  // Everything below comes from a draft the scan derived, so "confirmed" would be
  // a claim nobody made.
  const draft = isDraftPlan(summary);

  const verified = onboarding?.verifiedFeatures || [];
  const retro = onboarding?.retroactiveFeatures || [];
  const missing = onboarding?.missing || [];
  const deferred = onboarding?.deferred || [];

  const kpis = [
    {
      label: "Product Status",
      value: summary?.status || "—",
      sub: productStatusSub(summary),
      tone: productStatusTone(summary),
      icon: "◈"
    },
    {
      label: "Primary Goals",
      value: String(goals.length),
      sub: goals.length ? (draft ? "from README, unconfirmed" : "confirmed") : "none recorded",
      tone: "blue",
      icon: "✦"
    },
    {
      label: "Target Users",
      value: String(users.length),
      sub: users.length ? users.map((u) => u.segment).join(" · ") : "none recorded",
      tone: "blue",
      icon: "◉"
    },
    {
      label: "Tech Stack",
      value: String(arch.length),
      sub: arch.length ? (draft ? "from manifests, unconfirmed" : "layers documented") : "none recorded",
      tone: "slate",
      icon: "⌘"
    },
    {
      label: "Verified Features",
      value: String(verified.length),
      sub: `${retro.length} retroactive`,
      tone: "green",
      icon: "✓"
    },
    {
      label: "Attention",
      value: String(missing.length + deferred.length),
      sub: missing.length ? `${missing.length} missing · ${deferred.length} deferred` : "all clear",
      tone: missing.length ? "red" : deferred.length ? "amber" : "green",
      icon: "!"
    }
  ];

  return `
    <div class="ov-kpi-strip">
      ${kpis
        .map(
          (k) => `
        <div class="ov-kpi tone-${k.tone}">
          <span class="ov-kpi-icon">${k.icon}</span>
          <span class="ov-kpi-value">${escapeHtml(k.value)}</span>
          <span class="ov-kpi-label">${escapeHtml(k.label)}</span>
          <span class="ov-kpi-sub">${escapeHtml(k.sub)}</span>
        </div>`
        )
        .join("")}
    </div>
  `;
}

/* Grouped panels — product plan, repository, onboarding. */
function renderOverviewPanels(onboarding, git, summary) {
  const sidePanels = [];
  sidePanels.push(renderRepositoryPanel(git));
  if (summary) sidePanels.push(renderOpenQuestionsPanel(summary));
  if (onboarding) sidePanels.push(renderOnboardingPanel(onboarding));

  const mainPanel = summary ? renderProductPlanPanel(summary) : "";

  return `
    <div class="ov-panels">
      <div class="ov-col ov-col-main">${mainPanel}</div>
      <div class="ov-col ov-col-side">${sidePanels.join("")}</div>
    </div>
  `;
}

function renderProductPlanPanel(summary) {
  const goals = summary.goals || [];
  const nonGoals = summary.nonGoals || [];
  const users = summary.targetUsers || [];
  const guidance = summary.guidance || [];
  const arch = summary.architecture || [];

  const bulletList = (items, icon) =>
    items.length
      ? `<ul class="detail-list">${items
          .map((i) => `<li><span>${icon}</span><span>${escapeHtml(i)}</span></li>`)
          .join("")}</ul>`
      : `<div class="ov-empty">None</div>`;

  return `
    <section class="ov-panel">
      <header class="ov-panel-head">
        <span class="ov-panel-icon">◇</span>
        <h3>${isDraftPlan(summary) ? "Product Brief (draft)" : "Product Plan"}</h3>
      </header>
      <div class="ov-panel-body">
        ${summary.problem ? `<div class="ov-callout"><span class="ov-callout-label">Problem</span><p>${escapeHtml(summary.problem)}</p></div>` : ""}
        <div class="detail-grid">
          <div>
            <div class="detail-block-label">Goals</div>
            ${bulletList(goals, "✓")}
          </div>
          <div>
            <div class="detail-block-label">Non-Goals</div>
            ${bulletList(nonGoals, "✕")}
          </div>
          <div>
            <div class="detail-block-label">Target Users</div>
            ${
              users.length
                ? `<ul class="detail-list">${users
                    .map(
                      (u) =>
                        `<li><span>◉</span><span><span class="li-key">${escapeHtml(u.segment)}</span>${
                          u.need ? ` <span class="li-meta">${escapeHtml(u.need)}</span>` : ""
                        }</span></li>`
                    )
                    .join("")}</ul>`
                : `<div class="ov-empty">None</div>`
            }
          </div>
          <div>
            <div class="detail-block-label">Confirmed Guidance</div>
            ${bulletList(guidance, "▸")}
          </div>
          <div>
            <div class="detail-block-label">Architecture</div>
            ${bulletList(arch, "⌘")}
          </div>
        </div>
      </div>
    </section>
  `;
}

function renderOpenQuestionsPanel(summary) {
  const openQuestions = summary?.openQuestions || [];
  if (!openQuestions.length) return "";

  return `
    <section class="ov-panel">
      <header class="ov-panel-head">
        <span class="ov-panel-icon">?</span>
        <h3>Open Questions</h3>
        <span class="ov-panel-tag">${openQuestions.length}</span>
      </header>
      <div class="ov-panel-body">
        <ul class="detail-list">
          ${openQuestions
            .map((q) => `<li><span>?</span><span>${escapeHtml(q)}</span></li>`)
            .join("")}
        </ul>
      </div>
    </section>
  `;
}

function renderRepositoryPanel(git) {
  const rows = [
    { label: "Branch", value: git?.branch || "—" },
    { label: "Commit", value: git?.commit || "—" },
    { label: "Merged From", value: git?.mergedFrom || "—" },
    // Three states, not two: git may have been unable to answer at all, and "Clean"
    // would be a claim nobody verified.
    {
      label: "Working Tree",
      value:
        git?.dirty === true
          ? "Uncommitted changes"
          : git?.dirty === false
            ? "Clean"
            : "Not checked"
    }
  ];

  return `
    <section class="ov-panel">
      <header class="ov-panel-head">
        <span class="ov-panel-icon">⎇</span>
        <h3>Repository</h3>
      </header>
      <div class="ov-panel-body">
        <dl class="ov-facts">
          ${rows
            .map(
              (r) => `
            <div class="ov-fact">
              <dt>${escapeHtml(r.label)}</dt>
              <dd>${escapeHtml(r.value)}</dd>
            </div>`
            )
            .join("")}
        </dl>
      </div>
    </section>
  `;
}

function renderOnboardingPanel(onboarding) {
  const missing = onboarding.missing || [];
  const deferred = onboarding.deferred || [];
  const migrated = onboarding.migrated || [];
  const skipped = onboarding.skipped || [];

  const list = (items, icon, render) =>
    items.length
      ? `<ul class="detail-list">${items.map((i) => `<li><span>${icon}</span><span>${render(i)}</span></li>`).join("")}</ul>`
      : `<div class="ov-empty">None</div>`;

  return `
    <section class="ov-panel">
      <header class="ov-panel-head">
        <span class="ov-panel-icon">◈</span>
        <h3>Onboarding</h3>
        <span class="ov-panel-tag">${escapeHtml(formatDate(onboarding.date))}</span>
      </header>
      <div class="ov-panel-body">
        <div class="detail-grid">
          <div>
            <div class="detail-block-label">Missing</div>
            ${list(missing, "✕", (m) => escapeHtml(m))}
          </div>
          <div>
            <div class="detail-block-label">Deferred</div>
            ${list(deferred, "⏸", (m) => escapeHtml(m))}
          </div>
          <div>
            <div class="detail-block-label">Migrated Features</div>
            ${list(
              migrated,
              "▸",
              (m) => {
                const src = m.from === "openspec" ? "OpenSpec" : m.from === "speckit" ? "Spec-Kit" : m.from;
                return `<span class="li-key">${escapeHtml(m.feature)}</span> <span class="li-meta">from ${escapeHtml(src)}</span>`;
              }
            )}
          </div>
          <div>
            <div class="detail-block-label">Skipped Migrations</div>
            ${list(
              skipped,
              "⏭",
              (s) => {
                const sys = s.system === "openspec" ? "OpenSpec" : s.system === "speckit" ? "Spec-Kit" : s.system;
                return `<span class="li-key">${escapeHtml(sys)}</span>${s.reason ? ` <span class="li-meta">${escapeHtml(s.reason)}</span>` : ""}`;
              }
            )}
          </div>
        </div>
        ${onboarding.notes ? `<div class="ov-callout"><span class="ov-callout-label">Notes</span><p>${escapeHtml(onboarding.notes)}</p></div>` : ""}
      </div>
    </section>
  `;
}

/* ============================================================
   PRODUCT PLAN
   ============================================================ */
function renderProduct(product, productType) {
  const div = document.createElement("div");

  if (!product || !product.exists) {
    div.className = "panel-loading";

    // The product plan is a DEFINITION step: it invents a product overview, mockup,
    // roadmap and backlog for something that does not exist yet. That is the
    // greenfield path. On a project adopted from existing code the content is
    // already in the repository, so pointing at the same command as though nothing
    // were there is misleading.
    const type = productType?.type || "unknown";

    if (type === "brownfield") {
      div.innerHTML = `
        No product plan yet.<br><br>
        <strong>Product Plan is the greenfield definition step</strong> — it invents a
        product overview, mockup, roadmap and backlog for a product that does not exist
        yet. This project was adopted from existing code, so that content is already in
        the repository.<br><br>
        Start with the onboarding scan, which derives a draft product brief from your
        README (falling back to the code) and writes it to
        <code>.sdrive/product/overview.md</code>: <code>sdrive onboard</code><br><br>
        Then <code>/sdrive:product-plan</code> reads that draft, asks only about what the
        repository cannot tell it, and merges — it does not overwrite it.
      `;
      return div;
    }

    if (type === "greenfield") {
      div.innerHTML = `No product plan yet. Run <code>/sdrive:product-plan</code> to create a product overview, conceptual mockup, roadmap, and feature backlog.`;
      return div;
    }

    div.innerHTML = `No product plan yet. Run <code>/sdrive:product-plan</code> to create a product overview, conceptual mockup, roadmap, and feature backlog — or <code>sdrive onboard</code> first if this project already has code.`;
    return div;
  }

  const backlog = product.backlog || [];
  const pending = backlog.filter((item) => /pending|backlog/i.test(item.status)).length;
  const deferred = backlog.filter((item) => /deferred/i.test(item.status)).length;

  const BACKLOG_PER_PAGE = 10;

  const backlogRow = (item) => {
    const tone = /deferred/i.test(item.status)
      ? "slate"
      : /pending|backlog/i.test(item.status)
      ? "amber"
      : "green";
    return `
        <tr>
          <td class="backlog-id">${escapeHtml(item.id)}</td>
          <td>${escapeHtml(item.feature)}</td>
          <td>${item.milestone ? `<span class="chip slate">${escapeHtml(item.milestone)}</span>` : "—"}</td>
          <td>${item.priority ? escapeHtml(item.priority) : "—"}</td>
          <td><span class="chip ${tone}">${escapeHtml(item.status)}</span></td>
        </tr>`;
  };

  const backlogSlice = paginate(backlog, "product-backlog", BACKLOG_PER_PAGE);
  const backlogRows = backlog.length
    ? backlogSlice.items.map(backlogRow).join("")
    : `<tr><td colspan="5" class="product-empty">No backlog items yet.</td></tr>`;

  div.innerHTML = `
    <div class="product-plan-wrap">
      <div class="stat-card tone-accent">
        <span class="stat-icon">▤</span>
        <span class="stat-label">Backlog Items</span>
        <span class="stat-value">${backlog.length}</span>
        <span class="stat-sub">product-level candidates</span>
      </div>
      <div class="stat-card tone-amber">
        <span class="stat-icon">⏳</span>
        <span class="stat-label">Pending</span>
        <span class="stat-value">${pending}</span>
        <span class="stat-sub">awaiting promotion</span>
      </div>
      <div class="stat-card tone-slate">
        <span class="stat-icon">💤</span>
        <span class="stat-label">Deferred</span>
        <span class="stat-value">${deferred}</span>
        <span class="stat-sub">parked for later</span>
      </div>
    </div>

    <div class="product-plan-panel">
      <div class="detail-block-label">Artifacts</div>
      <div class="product-plan-artifacts">
        ${product.overview ? `<span class="artifact-chip">📄 overview.md</span>` : ""}
        ${product.mockup ? `<span class="artifact-chip">🗺️ mockup.md</span>` : ""}
        ${product.designSystem ? `<span class="artifact-chip">🎨 design-system.md</span>` : ""}
        ${product.roadmap ? `<span class="artifact-chip">🛣️ roadmap.md</span>` : ""}
      </div>

      <div class="table-scroll">
        <table class="backlog-table">
          <thead>
            <tr><th>ID</th><th>Feature</th><th>Milestone</th><th>Priority</th><th>Status</th></tr>
          </thead>
          <tbody id="backlog-body">${backlogRows}</tbody>
        </table>
      </div>

      <div id="backlog-pager"></div>

      <div class="product-note">
        Backlog items are product-level candidates. Promote one with <code>/sdrive:propose</code> to start detailed planning.
      </div>
    </div>
  `;

  // Re-render only the table body + pager when the page changes, so the
  // stat cards and artifact chips are never rebuilt.
  const pagerHost = div.querySelector("#backlog-pager");
  const bodyHost = div.querySelector("#backlog-body");
  const repaint = () => {
    const slice = paginate(backlog, "product-backlog", BACKLOG_PER_PAGE);
    bodyHost.innerHTML = slice.items.map(backlogRow).join("");
    pagerHost.innerHTML = "";
    const pager = renderPager(backlog.length, "product-backlog", BACKLOG_PER_PAGE, repaint);
    if (pager) pagerHost.appendChild(pager);
  };
  repaint();

  return div;
}

/* ============================================================
   FEATURES
   ============================================================ */
function renderVersionBlock(feature, version, isCurrent, traceability, verification, tests) {
  const rows = traceability.rows || [];
  const completed = rows.filter((r) => r.status === "complete").length;
  const coverage = rows.length === 0 ? 0 : Math.round((completed / rows.length) * 100);

  const vStatus = verification.status || "not_run";
  const vTone = verificationTone(vStatus);
  const vLabel =
    vStatus === "pass"
      ? `PASS · ${verification.summary?.matched ?? 0} matched`
      : vStatus === "warn"
      ? `WARN · ${verification.summary?.extra ?? 0} extra`
      : vStatus === "fail"
      ? `FAIL · ${verification.summary?.missing ?? 0} missing`
      : "NOT RUN";

  const tStatus = tests.status || "not_run";
  const tTone = tStatus === "complete" ? "green" : tStatus === "partial" ? "amber" : "slate";
  const tLabel =
    tStatus === "not_run"
      ? "NOT RUN"
      : `${tests.summary?.coverage ?? 0}% · ${tests.summary?.testedACs ?? 0}/${tests.summary?.totalACs ?? 0} ACs`;

  const history = (version.history || []).filter((h) => h.event.endsWith("_approved"));

  const block = document.createElement("div");
  block.className = "version-block" + (isCurrent ? " current" : "");

  block.innerHTML = `
    <div class="version-head">
      <span class="version-id">
        <span class="chip ${isCurrent ? "accent" : "slate"}">${isCurrent ? "current" : "past"}</span>
        ${escapeHtml(version.id)}
      </span>
      <span class="version-phase">${escapeHtml(version.phase)} · ${escapeHtml(version.currentStep)}</span>
    </div>

    <div class="gate-section">
      <div class="gate-section-head">
        <span class="gate-section-label">Gates</span>
        <span class="gate-section-hint" title="Approval checkpoints a feature must clear before it can advance">approval checkpoints</span>
      </div>
      <div class="gate-row">
        ${GATE_DEFS.map((g) => {
          const status = version.gates?.[g.key];
          const meta = gateStatusMeta(status);
          return `<span class="${gateClass(status)}" title="${escapeHtml(
            g.hint
          )}"><span class="gate-icon">${meta.icon}</span>${escapeHtml(
            g.label
          )}<span class="gate-status">${meta.text}</span></span>`;
        }).join("")}
      </div>
    </div>

    <div class="metric-row">
      <div class="metric">
        <span class="metric-label">Traceability</span>
        <span class="metric-bar ${coverage === 100 ? "green" : coverage > 0 ? "amber" : "red"}">
          <span style="width: ${coverage}%"></span>
        </span>
        <span class="metric-value">${coverage}% (${completed}/${rows.length})</span>
      </div>
      <div class="metric">
        <span class="metric-label">Verification</span>
        <span class="chip ${vTone}">${escapeHtml(vLabel)}</span>
      </div>
      <div class="metric">
        <span class="metric-label">Tests</span>
        <span class="chip ${tTone}">${escapeHtml(tLabel)}</span>
      </div>
    </div>

    ${
      history.length
        ? `<div class="history-list">${history
            .map(
              (h) =>
                `<div class="history-item"><span class="tick">✓</span>${escapeHtml(
                  h.event.replace("_approved", "")
                )} approved by ${escapeHtml(h.by || "unknown")}</div>`
            )
            .join("")}</div>`
        : ""
    }
  `;

  return block;
}

function featureMembers(feature, team) {
  const byRole = {};
  (team.members || []).forEach((m) => {
    byRole[m.username] = m.role;
  });

  const seen = new Map();
  const add = (h) => {
    if (h && h.by && !seen.has(h.by)) {
      seen.set(h.by, byRole[h.by] || "member");
    }
  };

  // History may live at the feature level or inside each version.
  (feature.history || []).forEach(add);
  (feature.versions || []).forEach((version) => {
    (version.history || []).forEach(add);
  });

  return Array.from(seen, ([username, role]) => ({ username, role }));
}

function roleFor(username, team) {
  const member = (team.members || []).find((m) => m.username === username);
  return member ? member.role : "member";
}

function featureAssigneeEl(feature, team) {
  if (!feature.assignee) return null;

  const role = roleFor(feature.assignee, team);
  const wrap = document.createElement("div");
  wrap.className = "feature-assignee";
  wrap.innerHTML = `
    <span class="feature-assignee-avatar role-${escapeHtml(role)}">${escapeHtml(
      initials(feature.assignee)
    )}</span>
    <span class="feature-assignee-info">
      <span class="feature-assignee-label">Assigned to</span>
      <span class="feature-assignee-name">${escapeHtml(feature.assignee)}</span>
    </span>
  `;
  return wrap;
}

function featureMembersEl(members) {
  const wrap = document.createElement("div");
  wrap.className = "feature-members";
  wrap.innerHTML = `
    <span class="feature-members-label">Approved by</span>
    <div class="feature-members-avatars">
      ${members
        .map(
          (m) =>
            `<span class="feature-member-avatar role-${escapeHtml(m.role)}" title="${escapeHtml(
              m.username
            )} · ${escapeHtml(m.role)}">${escapeHtml(initials(m.username))}</span>`
        )
        .join("")}
    </div>
  `;
  return wrap;
}

// Monotonic token used to cancel stale renders. renderFeatures() is async and
// awaits network calls inside its loop, so two overlapping invocations (e.g. a
// <select> firing both `input` and `change`) would each clear the container and
// then both append their cards, producing duplicate cards. Each render claims a
// token; any render that finds the token has moved on aborts before mutating the DOM.
let featureRenderToken = 0;

// Feature cards are tall (each carries its version blocks and metrics), so a
// smaller page keeps the grid scannable and the KPI explainer within reach.
const FEATURES_PER_PAGE = 20;

async function renderFeatures() {
  const token = ++featureRenderToken;

  const state = await fetchJson("/api/workflow-state");
  const team = (await fetchJson("/api/team")) || { members: [] };
  const container = document.getElementById("feature-list");
  const pagerHost = document.getElementById("feature-pager");

  if (token !== featureRenderToken) return;

  if (!state || !state.features || state.features.length === 0) {
    container.className = "panel-loading";
    container.textContent = "No features yet.";
    if (pagerHost) pagerHost.innerHTML = "";
    return;
  }

  const filters = getFilterValues();
  container.className = "feature-grid";
  container.innerHTML = "";
  if (pagerHost) pagerHost.innerHTML = "";

  // Build every matching card first, then paginate. Filtering depends on
  // per-version data fetched inside the loop, so we cannot slice the feature
  // list up front — we must know how many cards actually match.
  const cards = [];

  for (const f of state.features) {
    const card = document.createElement("div");
    card.className = "feature-card";

    const members = featureMembers(f, team);

    const head = document.createElement("div");
    head.className = "feature-card-head";
    head.innerHTML = `
      <div class="feature-card-title">
        <span class="feature-name">${escapeHtml(f.name)}</span>
        <span class="chip accent">${escapeHtml(f.currentVersion)}</span>
      </div>
      ${f.branch ? `<span class="feature-branch">⎇ ${escapeHtml(f.branch)}</span>` : ""}
    `;
    const assigneeEl = featureAssigneeEl(f, team);
    if (assigneeEl) {
      head.appendChild(assigneeEl);
    }
    if (members.length) {
      head.appendChild(featureMembersEl(members));
    }
    card.appendChild(head);

    const body = document.createElement("div");
    body.className = "feature-card-body";

    const versions = f.versions || [];
    let versionRendered = 0;

    for (const version of versions) {
      const isCurrent = version.id === f.currentVersion;

      if (filters.currentVersionOnly && !isCurrent) {
        continue;
      }

      const traceability = (await fetchJson(`/api/traceability/${f.name}/${version.id}`)) || { rows: [] };
      const verification = (await fetchJson(`/api/verification/${f.name}/${version.id}`)) || { status: "not_run", summary: {} };
      const tests = (await fetchJson(`/api/tests/${f.name}/${version.id}`)) || { status: "not_run", summary: {} };

      // A newer render started while we were awaiting — abandon this one.
      if (token !== featureRenderToken) return;

      if (!featureMatchesFilters(f, version, filters, verification, tests)) {
        continue;
      }

      body.appendChild(renderVersionBlock(f, version, isCurrent, traceability, verification, tests));
      versionRendered++;
    }

    if (versionRendered > 0) {
      card.appendChild(body);

      // Footer with a "View more" affordance that navigates to the
      // feature's own detail page (/features/<name>).
      const foot = document.createElement("div");
      foot.className = "feature-card-foot";
      const moreBtn = document.createElement("button");
      moreBtn.type = "button";
      moreBtn.className = "feature-more-btn";
      moreBtn.innerHTML = `View more <span class="feature-more-arrow">→</span>`;
      moreBtn.addEventListener("click", () => {
        const path = `/features/${encodeURIComponent(f.name)}`;
        window.history.pushState({ route: "feature-detail", feature: f.name }, "", path);
        showRoute("feature-detail");
        openFeatureDetail(f.name);
      });
      foot.appendChild(moreBtn);
      card.appendChild(foot);

      cards.push(card);
    }
  }

  if (token !== featureRenderToken) return;

  if (cards.length === 0) {
    container.className = "panel-loading";
    container.textContent = "No features match the current filters.";
    return;
  }

  // Paint the current page of already-built cards. Re-running this on a page
  // change only moves nodes around — no refetching, no rebuilding.
  const repaint = () => {
    const slice = paginate(cards, "features", FEATURES_PER_PAGE);
    container.innerHTML = "";
    slice.items.forEach((card) => container.appendChild(card));

    if (!pagerHost) return;
    pagerHost.innerHTML = "";
    const pager = renderPager(cards.length, "features", FEATURES_PER_PAGE, repaint);
    if (pager) pagerHost.appendChild(pager);
  };

  repaint();
}

/* ============================================================
   FEATURE DETAIL PAGE
   Reached from the "View more" button on a feature card. Instead
   of an overlay, this is its own routed page (/features/<name>)
   with filters for task status (pending / completed), version,
   phase, and a free-text search.
   ============================================================ */
let detailFeature = null;
let detailTeam = { members: [] };
let detailData = null;
let detailRenderToken = 0;

function detailFilters() {
  return {
    query: (document.getElementById("detail-search")?.value || "").trim().toLowerCase(),
    status: document.getElementById("detail-status")?.value || "",
    version: document.getElementById("detail-version")?.value || "",
    phase: document.getElementById("detail-phase")?.value || ""
  };
}

async function openFeatureDetail(featureName) {
  const state = await fetchJson("/api/workflow-state");
  const team = (await fetchJson("/api/team")) || { members: [] };
  const feature = (state?.features || []).find((f) => f.name === featureName);

  if (!feature) {
    detailFeature = null;
    detailData = null;
    const body = document.getElementById("detail-body");
    if (body) {
      body.innerHTML = `<div class="panel-loading">Feature “${escapeHtml(
        featureName
      )}” was not found.</div>`;
    }
    return;
  }

  detailFeature = feature;
  detailTeam = team;

  const title = document.getElementById("detail-title");
  const subtitle = document.getElementById("detail-subtitle");
  if (title) title.textContent = feature.name;
  if (subtitle) {
    subtitle.textContent = `${feature.currentVersion} · ${feature.phase || ""} · ${
      feature.currentStep || ""
    }`;
  }

  // Load every version's task breakdown once, then filter client-side.
  const versions = feature.versions || [];
  const versionData = [];
  for (const version of versions) {
    const tasks = (await fetchJson(`/api/tasks/${feature.name}/${version.id}`)) || {
      summary: { total: 0, completed: 0, pending: 0, percent: 0 },
      phases: []
    };
    versionData.push({ version, tasks });
  }
  detailData = { versions: versionData };

  populateDetailFilters(versionData);
  renderDetailSummary();
  renderDetailBody();
  renderDetailReports();
  renderDetailSpecs();
  renderDetailEvidence();
}

/* ============================================================
   FEATURE DETAIL — REPORTS PANEL
   Surfaces the artifacts each agent writes to .sdrive/reports/.
   Only reports/tests/ was previously wired up; this covers every
   report type (tests, performance, security, reviews, docs, debug)
   through the generic /api/reports/<feature>/<version> endpoint.
   ============================================================ */
async function renderDetailReports() {
  const container = document.getElementById("detail-reports");
  if (!container || !detailFeature) return;

  const version = detailFeature.currentVersion;
  const data = await fetchJson(`/api/reports/${detailFeature.name}/${version}`);

  if (!data || !Array.isArray(data.types)) {
    container.innerHTML = `<div class="panel-loading">No reports available.</div>`;
    return;
  }

  const present = data.types.filter((t) => t.status === "present");
  const missing = data.types.filter((t) => t.status !== "present");

  setDetailTabCount("reports", present.length);

  const cards = present
    .map((type) => {
      const rows = type.reports
        .map((r) => {
          const statusChip = r.status
            ? `<span class="chip ${reportStatusClass(r.status)}">${escapeHtml(r.status)}</span>`
            : "";
          const scopeChip =
            r.scope === "all"
              ? `<span class="chip slate" title="Covers every ongoing feature">all features</span>`
              : "";
          const historyChip =
            r.history > 1
              ? `<span class="chip slate" title="${r.history} reports on file">${r.history} on file</span>`
              : "";
          return `
            <li class="report-row">
              <div class="report-row-main">
                <span class="report-file">${escapeHtml(r.file)}</span>
                ${r.title ? `<span class="report-title">${escapeHtml(r.title)}</span>` : ""}
              </div>
              <div class="report-row-meta">
                ${statusChip}
                ${scopeChip}
                ${historyChip}
                <span class="report-date">${escapeHtml(r.date)}</span>
              </div>
            </li>
          `;
        })
        .join("");

      return `
        <div class="report-card">
          <div class="report-card-head">
            <span class="report-card-label">${escapeHtml(type.label)}</span>
            <span class="chip green">${type.reports.length}</span>
          </div>
          <ul class="report-list">${rows}</ul>
        </div>
      `;
    })
    .join("");

  const missingNote = missing.length
    ? `<div class="report-missing">
         <span class="report-missing-label">Not yet produced:</span>
         ${missing.map((t) => `<span class="chip slate">${escapeHtml(t.label)}</span>`).join("")}
       </div>`
    : "";

  container.innerHTML = present.length
    ? `<div class="report-grid">${cards}</div>${missingNote}`
    : `<div class="panel-loading">No reports have been produced for this feature yet.</div>${missingNote}`;
}

function reportStatusClass(status) {
  const s = (status || "").toUpperCase();
  if (s === "PASS" || s === "PASSED" || s === "CLEAN" || s === "IMPROVED") return "green";
  if (s === "FAIL" || s === "FAILED" || s === "REGRESSED") return "red";
  return "slate";
}

/* ============================================================
   FEATURE DETAIL — SPEC FILES PANEL
   Reads the raw spec/plan/tasks/traceability markdown straight
   from .sdrive/specs/<stage>/<feature>/<version>/ so the spec
   can be reviewed in the dashboard instead of the repo.
   ============================================================ */
let specFilesData = null;
let activeSpecDoc = null;

async function renderDetailSpecs() {
  const container = document.getElementById("detail-specs");
  if (!container || !detailFeature) return;

  const version = detailFeature.currentVersion;
  const data = await fetchJson(`/api/spec-files/${detailFeature.name}/${version}`);
  specFilesData = data;

  if (!data || !Array.isArray(data.docs) || !data.docs.length) {
    setDetailTabCount("specs", 0);
    container.innerHTML = `<div class="panel-loading">No spec files found for this feature.</div>`;
    return;
  }

  setDetailTabCount("specs", data.docs.length);

  // Default to the specification, falling back to the first available doc.
  const initial =
    data.docs.find((d) => d.id === "spec") || data.docs[0];
  activeSpecDoc = initial.id;

  const stageChip = data.stage
    ? `<span class="chip slate" title="Spec stage">${escapeHtml(data.stage)}</span>`
    : "";

  container.innerHTML = `
    <div class="spec-files">
      <aside class="spec-files-nav">
        <div class="spec-files-nav-head">
          <span class="spec-files-nav-label">Documents</span>
          ${stageChip}
        </div>
        <ul class="spec-doc-list">
          ${data.docs
            .map(
              (d) => `
            <li>
              <button
                class="spec-doc-btn${d.id === activeSpecDoc ? " active" : ""}"
                type="button"
                data-spec-doc="${escapeHtml(d.id)}"
              >
                <span class="spec-doc-name">${escapeHtml(d.label)}</span>
                <span class="spec-doc-meta">${d.lines} lines</span>
              </button>
            </li>
          `
            )
            .join("")}
        </ul>
      </aside>
      <div class="spec-files-view">
        <div class="spec-files-view-head">
          <code class="spec-files-path" id="spec-files-path">${escapeHtml(initial.path)}</code>
          <button class="spec-copy-btn" id="spec-copy-btn" type="button">Copy</button>
        </div>
        <div class="spec-search" id="spec-search">
          <div class="spec-search-field">
            <svg class="spec-search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <circle cx="11" cy="11" r="7"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            <input
              type="text"
              id="spec-search-input"
              class="spec-search-input"
              placeholder="Search in document"
              autocomplete="off"
              spellcheck="false"
              aria-label="Search in document"
            />
            <span class="spec-search-count" id="spec-search-count"></span>
            <button class="spec-search-nav" id="spec-search-prev" type="button" title="Previous match" aria-label="Previous match" disabled>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="18 15 12 9 6 15"></polyline></svg>
            </button>
            <button class="spec-search-nav" id="spec-search-next" type="button" title="Next match" aria-label="Next match" disabled>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="6 9 12 15 18 9"></polyline></svg>
            </button>
            <button class="spec-search-close" id="spec-search-close" type="button" title="Clear search" aria-label="Clear search" hidden>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
            </button>
          </div>
        </div>
        <article class="spec-markdown" id="spec-markdown">
          ${renderMarkdown(initial.content)}
        </article>
      </div>
    </div>
  `;

  container.querySelectorAll("[data-spec-doc]").forEach((btn) => {
    btn.addEventListener("click", () => selectSpecDoc(btn.dataset.specDoc));
  });

  const copyBtn = document.getElementById("spec-copy-btn");
  if (copyBtn) {
    copyBtn.addEventListener("click", () => {
      const doc = currentSpecDoc();
      if (!doc || !navigator.clipboard) return;
      navigator.clipboard.writeText(doc.content).then(() => {
        copyBtn.textContent = "Copied!";
        setTimeout(() => (copyBtn.textContent = "Copy"), 900);
      });
    });
  }

  setupSpecSearch();
  setupSpecFileLinks();
}

function currentSpecDoc() {
  if (!specFilesData || !Array.isArray(specFilesData.docs)) return null;
  return specFilesData.docs.find((d) => d.id === activeSpecDoc) || null;
}

function selectSpecDoc(id) {
  const doc = specFilesData && specFilesData.docs.find((d) => d.id === id);
  if (!doc) return;
  activeSpecDoc = id;

  document.querySelectorAll("[data-spec-doc]").forEach((btn) => {
    btn.classList.toggle("active", btn.dataset.specDoc === id);
  });

  const pathEl = document.getElementById("spec-files-path");
  if (pathEl) pathEl.textContent = doc.path;

  const md = document.getElementById("spec-markdown");
  if (md) md.innerHTML = renderMarkdown(doc.content);

  // Re-apply the active search to the newly rendered document, and re-bind the path
  // links that the re-render just replaced.
  runSpecSearch();
  setupSpecFileLinks();
}

/* ============================================================
   SPEC FILES — CLICKABLE PATH REFERENCES + FILE MODAL
   Inline code in a rendered spec is shown in the accent colour, and much of it is
   a real path. Making those clickable lets you read the file a requirement refers
   to without leaving the dashboard. Only path-shaped, space-free text is offered:
   fenced blocks are skipped, and so are slash commands and absolute paths.

   The modal is large on purpose (85% x 90%) - these are documents, not tooltips -
   and it carries the same find-in-document behaviour as the Spec Files view:
   highlight every match, step through them, report the count.
   ============================================================ */
const SPEC_PATH_SHAPE = /^[\w.@()[\],+-]+(?:[/\\][\w.@()[\],+-]+)+$/;

function looksLikePathReference(text) {
  const value = text.trim();
  if (!value || value.length > 240) return false;
  if (/\s/.test(value)) return false;
  if (value.startsWith("/") || value.startsWith("~")) return false;
  if (/^[a-z]+:\/\//i.test(value)) return false;
  return SPEC_PATH_SHAPE.test(value);
}

async function setupSpecFileLinks() {
  const md = document.getElementById("spec-markdown");
  if (!md) return;

  const candidates = Array.from(md.querySelectorAll("code")).filter(
    (code) =>
      !code.closest(".spec-code") &&
      !code.dataset.fileLink &&
      looksLikePathReference(code.textContent)
  );
  if (!candidates.length) return;

  // Only real files get the underline. A spec is full of path-SHAPED tokens that
  // are not files at all (`bg-muted/30`, `--shadow-sm/md`), and marking those
  // clickable is a lie the reader pays for with a "no such file" click.
  const exists = await Promise.all(
    candidates.map((code) =>
      fetchJson(`/api/file?check=1&path=${encodeURIComponent(code.textContent.trim())}`)
        .then((result) => Boolean(result && result.found))
        .catch(() => false)
    )
  );

  candidates.forEach((code, index) => {
    // A different document may have rendered while we were awaiting.
    if (!exists[index] || !md.contains(code)) return;

    code.dataset.fileLink = "1";
    code.classList.add("spec-file-link");
    code.setAttribute("role", "button");
    code.setAttribute("tabindex", "0");
    code.title = `Open ${code.textContent.trim()}`;

    const open = () => openFileModal(code.textContent.trim());
    code.addEventListener("click", open);
    code.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        open();
      }
    });
  });
}

let fileModalMatches = [];
let fileModalIndex = -1;
let fileModalContent = "";

function ensureFileModal() {
  let overlay = document.getElementById("file-modal");
  if (overlay) return overlay;

  overlay = document.createElement("div");
  overlay.id = "file-modal";
  overlay.className = "file-modal";
  overlay.innerHTML = `
    <div class="file-modal-backdrop" data-file-modal-close></div>
    <div class="file-modal-panel" role="dialog" aria-modal="true" aria-labelledby="file-modal-path">
      <header class="file-modal-head">
        <code class="file-modal-path" id="file-modal-path"></code>
        <span class="file-modal-meta" id="file-modal-meta"></span>
        <div class="file-modal-actions">
          <button class="spec-copy-btn" id="file-modal-copy" type="button">Copy</button>
          <button class="file-modal-close" id="file-modal-close" type="button" aria-label="Close file">×</button>
        </div>
      </header>
      <div class="spec-search">
        <div class="spec-search-field">
          <input type="text" id="file-modal-search" class="spec-search-input" placeholder="Search in file" autocomplete="off" spellcheck="false" aria-label="Search in file" />
          <span class="spec-search-count" id="file-modal-count"></span>
          <button class="spec-search-nav" id="file-modal-prev" type="button" title="Previous match" disabled>‹</button>
          <button class="spec-search-nav" id="file-modal-next" type="button" title="Next match" disabled>›</button>
          <button class="spec-search-close" id="file-modal-clear" type="button" title="Clear search" hidden>×</button>
        </div>
      </div>
      <div class="file-modal-body" id="file-modal-body"></div>
    </div>
  `;
  document.body.appendChild(overlay);

  overlay.querySelector("#file-modal-close").addEventListener("click", closeFileModal);
  overlay.querySelectorAll("[data-file-modal-close]").forEach((el) => {
    el.addEventListener("click", closeFileModal);
  });

  const copyBtn = overlay.querySelector("#file-modal-copy");
  copyBtn.addEventListener("click", () => {
    if (!navigator.clipboard) return;
    navigator.clipboard.writeText(fileModalContent).then(() => {
      copyBtn.textContent = "Copied!";
      setTimeout(() => (copyBtn.textContent = "Copy"), 900);
    });
  });

  const input = overlay.querySelector("#file-modal-search");
  input.addEventListener("input", runFileModalSearch);
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      stepFileModalMatch(event.shiftKey ? -1 : 1);
    }
    if (event.key === "Escape") closeFileModal();
  });

  overlay.querySelector("#file-modal-prev").addEventListener("click", () => stepFileModalMatch(-1));
  overlay.querySelector("#file-modal-next").addEventListener("click", () => stepFileModalMatch(1));
  overlay.querySelector("#file-modal-clear").addEventListener("click", () => {
    input.value = "";
    runFileModalSearch();
    input.focus();
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && document.getElementById("file-modal")) closeFileModal();
  });

  return overlay;
}

function closeFileModal() {
  const overlay = document.getElementById("file-modal");
  if (overlay) overlay.remove();
  fileModalMatches = [];
  fileModalIndex = -1;
}

function updateFileModalCount() {
  const count = document.getElementById("file-modal-count");
  const input = document.getElementById("file-modal-search");
  if (!count || !input) return;

  if (input.value.trim().length < 2) {
    count.textContent = "";
    return;
  }
  if (!fileModalMatches.length) {
    count.textContent = "no matches";
    return;
  }
  count.textContent = `${fileModalIndex + 1} of ${fileModalMatches.length}`;
}

function stripFileModalMarks() {
  const body = document.getElementById("file-modal-body");
  if (!body) return;

  body.querySelectorAll("mark.spec-hit").forEach((mark) => {
    const parent = mark.parentNode;
    parent.replaceChild(document.createTextNode(mark.textContent), mark);
    parent.normalize();
  });
}

function runFileModalSearch() {
  const body = document.getElementById("file-modal-body");
  const input = document.getElementById("file-modal-search");
  const clear = document.getElementById("file-modal-clear");
  const prev = document.getElementById("file-modal-prev");
  const next = document.getElementById("file-modal-next");
  if (!body || !input) return;

  stripFileModalMarks();
  fileModalMatches = [];
  fileModalIndex = -1;

  const query = input.value.trim();
  if (clear) clear.hidden = query.length === 0;

  if (query.length >= 2) {
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT, null);
    const nodes = [];
    while (walker.nextNode()) nodes.push(walker.currentNode);

    const needle = query.toLowerCase();
    for (const node of nodes) {
      const text = node.nodeValue;
      const lower = text.toLowerCase();
      let at = lower.indexOf(needle);
      if (at === -1) continue;

      const frag = document.createDocumentFragment();
      let cursor = 0;
      while (at !== -1) {
        if (at > cursor) frag.appendChild(document.createTextNode(text.slice(cursor, at)));
        const mark = document.createElement("mark");
        mark.className = "spec-hit";
        mark.textContent = text.slice(at, at + query.length);
        frag.appendChild(mark);
        fileModalMatches.push(mark);
        cursor = at + query.length;
        at = lower.indexOf(needle, cursor);
      }
      if (cursor < text.length) frag.appendChild(document.createTextNode(text.slice(cursor)));
      node.parentNode.replaceChild(frag, node);
    }
  }

  const has = fileModalMatches.length > 0;
  if (prev) prev.disabled = !has;
  if (next) next.disabled = !has;

  if (has) {
    fileModalIndex = 0;
    fileModalMatches[0].classList.add("active");
    fileModalMatches[0].scrollIntoView({ block: "center", behavior: "smooth" });
  }

  updateFileModalCount();
}

function stepFileModalMatch(direction) {
  if (!fileModalMatches.length) return;

  fileModalMatches.forEach((mark) => mark.classList.remove("active"));
  fileModalIndex =
    (fileModalIndex + direction + fileModalMatches.length) % fileModalMatches.length;

  const active = fileModalMatches[fileModalIndex];
  active.classList.add("active");
  active.scrollIntoView({ block: "center", behavior: "smooth" });
  updateFileModalCount();
}

async function openFileModal(relPath) {
  const overlay = ensureFileModal();
  const pathEl = overlay.querySelector("#file-modal-path");
  const metaEl = overlay.querySelector("#file-modal-meta");
  const body = overlay.querySelector("#file-modal-body");
  const input = overlay.querySelector("#file-modal-search");

  pathEl.textContent = relPath;
  metaEl.textContent = "loading…";
  body.innerHTML = `<div class="panel-loading">Loading…</div>`;
  input.value = "";
  fileModalMatches = [];
  fileModalIndex = -1;
  updateFileModalCount();

  const data = await fetchJson(`/api/file?path=${encodeURIComponent(relPath)}`);

  if (!data || !data.found) {
    metaEl.textContent = "";
    body.innerHTML = `<div class="panel-loading">Cannot open <code>${escapeHtml(relPath)}</code>: ${escapeHtml(
      (data && data.reason) || "not found"
    )}</div>`;
    return;
  }

  if (data.binary) {
    metaEl.textContent = `${data.bytes} bytes`;
    body.innerHTML = `<div class="panel-loading">Binary file - nothing to display.</div>`;
    return;
  }

  fileModalContent = data.content;
  metaEl.textContent = `${data.lines} lines · ${data.bytes} bytes${data.truncated ? " · truncated" : ""}`;

  body.innerHTML = data.markdown
    ? `<article class="spec-markdown">${renderMarkdown(data.content)}</article>`
    : `<pre>${escapeHtml(data.content)}</pre>`;

  input.focus();
}

/* ============================================================
   SPEC FILES — IN-DOCUMENT SEARCH
   A VS Code-style find bar: type a query, every occurrence is
   highlighted, and the up/down arrows step through the matches.
   ============================================================ */
let specSearchMatches = [];
let specSearchIndex = -1;

function setupSpecSearch() {
  const input = document.getElementById("spec-search-input");
  const prev = document.getElementById("spec-search-prev");
  const next = document.getElementById("spec-search-next");
  const close = document.getElementById("spec-search-close");
  if (!input) return;

  input.addEventListener("input", () => {
    runSpecSearch();
  });

  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      stepSpecSearch(e.shiftKey ? -1 : 1);
    } else if (e.key === "Escape") {
      e.preventDefault();
      clearSpecSearch();
    }
  });

  if (prev) prev.addEventListener("click", () => stepSpecSearch(-1));
  if (next) next.addEventListener("click", () => stepSpecSearch(1));
  if (close) close.addEventListener("click", clearSpecSearch);
}

function clearSpecSearch() {
  const input = document.getElementById("spec-search-input");
  if (input) input.value = "";
  runSpecSearch();
  if (input) input.focus();
}

/* Walks the rendered markdown, wraps every case-insensitive occurrence of the
   query in a <mark>, and records the marks so the arrows can step through them.
   Text nodes are visited in document order, so match order matches reading
   order. Existing highlights are unwrapped first so re-running is idempotent. */
function runSpecSearch() {
  const input = document.getElementById("spec-search-input");
  const countEl = document.getElementById("spec-search-count");
  const prev = document.getElementById("spec-search-prev");
  const next = document.getElementById("spec-search-next");
  const close = document.getElementById("spec-search-close");
  const md = document.getElementById("spec-markdown");
  if (!input || !md) return;

  // Remove any previous highlights and restore the original text.
  md.querySelectorAll("mark.spec-hit").forEach((mark) => {
    const parent = mark.parentNode;
    if (!parent) return;
    parent.replaceChild(document.createTextNode(mark.textContent), mark);
    parent.normalize();
  });

  specSearchMatches = [];
  specSearchIndex = -1;

  const query = input.value.trim();
  if (close) close.hidden = !query;

  if (!query) {
    if (countEl) countEl.textContent = "";
    if (prev) prev.disabled = true;
    if (next) next.disabled = true;
    return;
  }

  const needle = query.toLowerCase();

  // Collect text nodes first — mutating the DOM while walking it is unsafe.
  const walker = document.createTreeWalker(md, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      if (!node.nodeValue || !node.nodeValue.trim()) return NodeFilter.FILTER_REJECT;
      const tag = node.parentNode && node.parentNode.nodeName;
      if (tag === "SCRIPT" || tag === "STYLE") return NodeFilter.FILTER_REJECT;
      return NodeFilter.FILTER_ACCEPT;
    },
  });

  const textNodes = [];
  let node;
  while ((node = walker.nextNode())) textNodes.push(node);

  textNodes.forEach((textNode) => {
    const text = textNode.nodeValue;
    const lower = text.toLowerCase();
    let from = 0;
    let idx = lower.indexOf(needle, from);
    if (idx === -1) return;

    const frag = document.createDocumentFragment();
    let cursor = 0;
    while (idx !== -1) {
      if (idx > cursor) {
        frag.appendChild(document.createTextNode(text.slice(cursor, idx)));
      }
      const mark = document.createElement("mark");
      mark.className = "spec-hit";
      mark.textContent = text.slice(idx, idx + query.length);
      frag.appendChild(mark);
      specSearchMatches.push(mark);
      cursor = idx + query.length;
      from = cursor;
      idx = lower.indexOf(needle, from);
    }
    if (cursor < text.length) {
      frag.appendChild(document.createTextNode(text.slice(cursor)));
    }
    textNode.parentNode.replaceChild(frag, textNode);
  });

  const total = specSearchMatches.length;
  if (countEl) {
    countEl.textContent = total ? `1 of ${total}` : "No results";
    countEl.classList.toggle("empty", total === 0);
  }
  if (prev) prev.disabled = total === 0;
  if (next) next.disabled = total === 0;

  if (total) {
    specSearchIndex = 0;
    focusSpecMatch();
  }
}

/* Moves the active highlight by `delta` (wrapping around) and scrolls it into
   view. The active mark gets a distinct class so it stands out from the rest. */
function stepSpecSearch(delta) {
  if (!specSearchMatches.length) return;
  specSearchIndex =
    (specSearchIndex + delta + specSearchMatches.length) % specSearchMatches.length;
  focusSpecMatch();
}

function focusSpecMatch() {
  const countEl = document.getElementById("spec-search-count");
  specSearchMatches.forEach((mark, i) => {
    mark.classList.toggle("active", i === specSearchIndex);
  });
  const active = specSearchMatches[specSearchIndex];
  if (active) {
    active.scrollIntoView({ block: "center", behavior: "smooth" });
  }
  if (countEl && specSearchMatches.length) {
    countEl.textContent = `${specSearchIndex + 1} of ${specSearchMatches.length}`;
  }
}

/* Minimal, dependency-free Markdown renderer. It covers the subset the spec
   documents actually use: headings, lists, tables, fenced code, blockquotes,
   horizontal rules, and inline bold/italic/code/links. Everything is escaped
   before any markup is added, so raw HTML in a spec file cannot inject. */
function renderMarkdown(src) {
  if (!src) return `<p class="spec-empty">This document is empty.</p>`;

  const lines = src.replace(/\r\n/g, "\n").split("\n");
  const out = [];
  let i = 0;

  const safeHref = (href) => {
    const value = String(href || "").trim();

    // Relative links and the three schemes the dashboard has any use for.
    // Everything else (notably javascript:) becomes inert.
    if (/^(https?:\/\/|mailto:|\/|\.{0,2}\/|#)/i.test(value)) return value;
    return "#";
  };

  const inline = (text) =>
    escapeHtml(text)
      .replace(/`([^`]+)`/g, "<code>$1</code>")
      .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
      .replace(/(^|[^*])\*([^*]+)\*/g, "$1<em>$2</em>")
      .replace(
        /\[([^\]]+)\]\(([^)]+)\)/g,
        (match, label, href) =>
          `<a href="${safeHref(href)}" target="_blank" rel="noopener noreferrer">${label}</a>`
      );

  while (i < lines.length) {
    const line = lines[i];

    // Fenced code block
    if (/^```/.test(line)) {
      const lang = line.slice(3).trim();
      const buf = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) {
        buf.push(lines[i]);
        i++;
      }
      i++; // closing fence
      out.push(
        `<pre class="spec-code"><code${lang ? ` data-lang="${escapeHtml(lang)}"` : ""}>${escapeHtml(
          buf.join("\n")
        )}</code></pre>`
      );
      continue;
    }

    // Heading
    const heading = line.match(/^(#{1,6})\s+(.*)$/);
    if (heading) {
      const level = heading[1].length;
      out.push(`<h${level} class="spec-h spec-h${level}">${inline(heading[2])}</h${level}>`);
      i++;
      continue;
    }

    // Horizontal rule
    if (/^\s*([-*_])\1{2,}\s*$/.test(line)) {
      out.push('<hr class="spec-hr" />');
      i++;
      continue;
    }

    // Table (header row followed by a separator row)
    if (
      /\|/.test(line) &&
      i + 1 < lines.length &&
      /^\s*\|?[\s:|-]+\|[\s:|-]*$/.test(lines[i + 1])
    ) {
      const parseRow = (row) =>
        row
          .replace(/^\s*\|/, "")
          .replace(/\|\s*$/, "")
          .split("|")
          .map((c) => c.trim());
      const headers = parseRow(line);
      i += 2;
      const rows = [];
      while (i < lines.length && /\|/.test(lines[i]) && lines[i].trim()) {
        rows.push(parseRow(lines[i]));
        i++;
      }
      out.push(`
        <div class="spec-table-wrap">
          <table class="spec-table">
            <thead><tr>${headers.map((h) => `<th>${inline(h)}</th>`).join("")}</tr></thead>
            <tbody>${rows
              .map((r) => `<tr>${r.map((c) => `<td>${inline(c)}</td>`).join("")}</tr>`)
              .join("")}</tbody>
          </table>
        </div>
      `);
      continue;
    }

    // Blockquote
    if (/^\s*>\s?/.test(line)) {
      const buf = [];
      while (i < lines.length && /^\s*>\s?/.test(lines[i])) {
        buf.push(lines[i].replace(/^\s*>\s?/, ""));
        i++;
      }
      out.push(`<blockquote class="spec-quote">${inline(buf.join(" "))}</blockquote>`);
      continue;
    }

    // Unordered list
    if (/^\s*[-*+]\s+/.test(line)) {
      const items = [];
      while (i < lines.length && /^\s*[-*+]\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*[-*+]\s+/, ""));
        i++;
      }
      out.push(`<ul class="spec-ul">${items.map((t) => `<li>${inline(t)}</li>`).join("")}</ul>`);
      continue;
    }

    // Ordered list
    if (/^\s*\d+[.)]\s+/.test(line)) {
      const items = [];
      while (i < lines.length && /^\s*\d+[.)]\s+/.test(lines[i])) {
        items.push(lines[i].replace(/^\s*\d+[.)]\s+/, ""));
        i++;
      }
      out.push(`<ol class="spec-ol">${items.map((t) => `<li>${inline(t)}</li>`).join("")}</ol>`);
      continue;
    }

    // Blank line
    if (!line.trim()) {
      i++;
      continue;
    }

    // Paragraph (gather consecutive plain lines)
    const buf = [];
    while (
      i < lines.length &&
      lines[i].trim() &&
      !/^(#{1,6}\s|```|\s*[-*+]\s|\s*\d+[.)]\s|\s*>)/.test(lines[i]) &&
      !/^\s*([-*_])\1{2,}\s*$/.test(lines[i])
    ) {
      buf.push(lines[i]);
      i++;
    }
    out.push(`<p class="spec-p">${inline(buf.join(" "))}</p>`);
  }

  return out.join("\n");
}

function populateDetailFilters(versionData) {
  const versionSelect = document.getElementById("detail-version");
  const phaseSelect = document.getElementById("detail-phase");
  if (!versionSelect || !phaseSelect) return;

  const prevVersion = versionSelect.value;
  const prevPhase = phaseSelect.value;

  versionSelect.innerHTML =
    `<option value="">All versions</option>` +
    versionData
      .map(({ version }) => `<option value="${escapeHtml(version.id)}">${escapeHtml(version.id)}</option>`)
      .join("");

  const phaseIds = new Set();
  versionData.forEach(({ tasks }) => {
    (tasks.phases || []).forEach((p) => phaseIds.add(p.phaseId));
  });
  phaseSelect.innerHTML =
    `<option value="">All phases</option>` +
    Array.from(phaseIds)
      .map((id) => `<option value="${escapeHtml(id)}">${escapeHtml(id)}</option>`)
      .join("");

  if (Array.from(versionSelect.options).some((o) => o.value === prevVersion)) {
    versionSelect.value = prevVersion;
  }
  if (Array.from(phaseSelect.options).some((o) => o.value === prevPhase)) {
    phaseSelect.value = prevPhase;
  }
}

async function renderDetailEvidence() {
  const container = document.getElementById("detail-evidence");
  if (!container || !detailFeature) return;

  const version = detailFeature.currentVersion;
  const feature = detailFeature.name;

  const [evidence, review, archive, git] = await Promise.all([
    fetchJson(`/api/test-evidence/${encodeURIComponent(feature)}/${encodeURIComponent(version)}`),
    fetchJson(`/api/review/${encodeURIComponent(feature)}/${encodeURIComponent(version)}`),
    fetchJson(`/api/archive-check/${encodeURIComponent(feature)}`),
    fetchJson("/api/git-info")
  ]);

  const cards = [];
  let count = 0;

  const evidenceValue = evidence && evidence.found ? evidence.evidence : null;
  const head = git && git.commit ? git.commit : "";

  if (evidenceValue) {
    count += 1;
    const summary = evidenceValue.summary || {};
    const passed = evidenceValue.exitCode === 0 && Number(summary.failed || 0) === 0;
    const stale = head && evidenceValue.commit && evidenceValue.commit !== head;
    cards.push(`
      <div class="card quality-card">
        <div class="detail-block-label">Test evidence</div>
        <div class="detail-list">
          <div><span class="chip ${passed ? "green" : "red"}">${passed ? "passing" : "failing"}</span>
            <span class="chip ${stale ? "amber" : "slate"}">${stale ? "stale" : "current"}</span></div>
          <div class="li-meta"><strong>Command:</strong> ${escapeHtml(evidenceValue.command || "")}</div>
          <div class="li-meta"><strong>Counts:</strong> ${Number(summary.passed || 0)} passed · ${Number(summary.failed || 0)} failed · ${Number(summary.skipped || 0)} skipped</div>
          <div class="li-meta"><strong>Commit:</strong> ${escapeHtml(String(evidenceValue.commit || "unknown").slice(0, 8))}${stale ? ` (HEAD ${escapeHtml(head.slice(0, 8))})` : ""}</div>
          <div class="li-meta"><strong>Duration:</strong> ${Number(evidenceValue.durationMs || 0)} ms</div>
        </div>
      </div>
    `);
  } else {
    cards.push(`
      <div class="card quality-card">
        <div class="detail-block-label">Test evidence</div>
        <div class="panel-loading">No test evidence yet. Run <code>sdrive test ${escapeHtml(feature)}</code>.</div>
      </div>
    `);
  }

  const reviewValue = review && review.found ? review.review : null;

  if (reviewValue) {
    count += 1;
    const findings = reviewValue.findings || [];
    cards.push(`
      <div class="card quality-card">
        <div class="detail-block-label">Structured review</div>
        <div class="detail-list">
          <div><span class="chip ${reviewValue.status === "PASS" ? "green" : "red"}">${escapeHtml(reviewValue.status || "unknown")}</span>
            <span class="chip slate">${Number(reviewValue.coverage || 0)}% coverage</span></div>
          <div class="li-meta"><strong>Reviewer:</strong> ${escapeHtml(reviewValue.reviewedBy || "unknown")}</div>
          <div class="li-meta"><strong>Commit:</strong> ${escapeHtml(String(reviewValue.commit || "unknown").slice(0, 8))}</div>
          <div class="li-meta"><strong>Findings:</strong> ${findings.length}</div>
          ${findings.length
            ? `<ul class="detail-list">${findings
                .slice(0, 8)
                .map(
                  (finding) =>
                    `<li><span class="chip ${finding.severity === "high" ? "red" : finding.severity === "medium" ? "amber" : "slate"}">${escapeHtml(finding.severity || "info")}</span> ${escapeHtml(finding.issue || "")} <span class="li-meta">${escapeHtml(finding.location || "")}</span></li>`
                )
                .join("")}</ul>`
            : ""}
        </div>
      </div>
    `);
  } else {
    cards.push(`
      <div class="card quality-card">
        <div class="detail-block-label">Structured review</div>
        <div class="panel-loading">No review.json yet. Run <code>/sdrive:review ${escapeHtml(feature)}</code>.</div>
      </div>
    `);
  }

  count += 1;
  const blockers = (archive && archive.blockers) || [];
  cards.push(`
    <div class="card quality-card">
      <div class="detail-block-label">Archive readiness</div>
      <div class="detail-list">
        <div><span class="chip ${archive && archive.ok ? "green" : "amber"}">${archive && archive.ok ? "archivable" : "blocked"}</span></div>
        ${
          blockers.length
            ? `<ul class="detail-list">${blockers.map((blocker) => `<li>${escapeHtml(blocker)}</li>`).join("")}</ul>`
            : `<div class="li-meta">All recorded gates and evidence are present.</div>`
        }
      </div>
    </div>
  `);

  container.innerHTML = cards.join("");
  setDetailTabCount("evidence", count);
}

async function renderQuality() {
  const container = document.getElementById("quality-panel");
  if (!container) return;

  const [validation, migration] = await Promise.all([
    fetchJson("/api/validate"),
    fetchJson("/api/migrations")
  ]);

  const results = validation?.results || [];
  const invalid = results.filter((result) => result.status === "invalid");
  const valid = results.filter((result) => result.status === "valid");
  const skipped = results.filter((result) => result.status === "skipped");
  const detections = migration?.detections || [];
  const runs = migration?.runs || [];

  container.innerHTML = `
    <div class="card quality-card">
      <div class="detail-block-label">Schema validation
        <span class="chip ${validation?.ok ? "green" : "red"}">${validation?.ok ? "valid" : "invalid"}</span>
      </div>
      ${
        validation?.error
          ? `<div class="panel-loading">Validation could not run: ${escapeHtml(validation.error)}</div>`
          : `<div class="detail-list">
              <div class="li-meta">${valid.length} valid · ${invalid.length} invalid · ${skipped.length} skipped</div>
              ${invalid.length
                ? `<ul class="detail-list">${invalid
                    .map(
                      (result) =>
                        `<li><strong>${escapeHtml(result.file)}</strong><ul class="detail-list">${(result.errors || [])
                          .slice(0, 6)
                          .map((error) => `<li class="li-meta">${escapeHtml(typeof error === "string" ? error : JSON.stringify(error))}</li>`)
                          .join("")}</ul></li>`
                    )
                    .join("")}</ul>`
                : `<div class="li-meta">No schema errors.</div>`}
            </div>`
      }
    </div>

    <div class="card quality-card">
      <div class="detail-block-label">Migration adapters</div>
      <div class="detail-list">
        ${
          detections.length
            ? detections
                .map(
                  (entry) =>
                    `<div><span class="chip blue">${escapeHtml(entry.id)}</span> ${escapeHtml(entry.name)} — ${escapeHtml(entry.root || "")}</div>`
                )
                .join("")
            : `<div class="li-meta">No supported external framework detected.</div>`
        }
      </div>
      <div class="detail-block-label" style="margin-top:16px">Recent migrations</div>
      <div class="detail-list">
        ${
          runs.length
            ? runs
                .map(
                  (run) =>
                    `<div><span class="chip slate">${escapeHtml(run.framework || "")}</span> ${escapeHtml(run.id)} — imported ${Number(run.counts?.imported || 0)}, skipped ${Number(run.counts?.skipped || 0)}, failed ${Number(run.counts?.failed || 0)}</div>`
                )
                .join("")
            : `<div class="li-meta">No migration runs yet. Plan one with <code>sdrive migrate:plan &lt;framework&gt;</code>.</div>`
        }
      </div>
    </div>
  `;
}

function renderDetailSummary() {
  const container = document.getElementById("detail-summary");
  if (!container || !detailData) return;

  let total = 0;
  let done = 0;
  detailData.versions.forEach(({ tasks }) => {
    total += tasks.summary?.total || 0;
    done += tasks.summary?.completed || 0;
  });
  const pending = total - done;
  const percent = total === 0 ? 0 : Math.round((done / total) * 100);

  container.innerHTML = `
    <div class="modal-stat">
      <span class="modal-stat-value">${total}</span>
      <span class="modal-stat-label">Total tasks</span>
    </div>
    <div class="modal-stat">
      <span class="modal-stat-value green">${done}</span>
      <span class="modal-stat-label">Completed</span>
    </div>
    <div class="modal-stat">
      <span class="modal-stat-value amber">${pending}</span>
      <span class="modal-stat-label">Pending</span>
    </div>
    <div class="modal-stat">
      <span class="modal-stat-value">${percent}%</span>
      <span class="modal-stat-label">Progress</span>
    </div>
  `;
}

function detailTaskMatches(task, filters) {
  if (filters.status === "pending" && task.completed) return false;
  if (filters.status === "completed" && !task.completed) return false;
  if (filters.query) {
    const haystack = `${task.id || ""} ${task.title || ""} ${task.description || ""}`.toLowerCase();
    if (!haystack.includes(filters.query)) return false;
  }
  return true;
}

function renderDetailBody() {
  const container = document.getElementById("detail-body");
  if (!container || !detailData) return;

  const filters = detailFilters();
  const frag = document.createDocumentFragment();
  let renderedVersions = 0;
  let totalVisible = 0;

  for (const { version, tasks } of detailData.versions) {
    if (filters.version && version.id !== filters.version) continue;

    const isCurrent = detailFeature && version.id === detailFeature.currentVersion;
    const s = tasks.summary || { total: 0, completed: 0, pending: 0, percent: 0 };

    const phaseBlocks = [];
    let versionTaskCount = 0;

    for (const p of tasks.phases || []) {
      if (filters.phase && p.phaseId !== filters.phase) continue;

      const visible = (p.tasks || []).filter((t) => detailTaskMatches(t, filters));
      if (!visible.length) continue;

      versionTaskCount += visible.length;
      const doneCount = visible.filter((t) => t.completed).length;

      phaseBlocks.push(`
        <div class="detail-phase">
          <div class="detail-phase-head">
            <span class="detail-phase-id">${escapeHtml(p.phaseId)}</span>
            <span class="detail-phase-name">${escapeHtml(p.name)}</span>
            <span class="detail-phase-count">${doneCount}/${visible.length}</span>
          </div>
          <ul class="detail-task-list">
            ${visible
              .map(
                (t) => `
              <li class="detail-task ${t.completed ? "done" : "pending"}">
                <span class="detail-task-check">${t.completed ? "✓" : "○"}</span>
                <div class="detail-task-body">
                  <div class="detail-task-title">
                    <span class="detail-task-id">${escapeHtml(t.id)}</span>
                    ${escapeHtml(t.title)}
                  </div>
                  ${
                    t.description
                      ? `<div class="detail-task-desc">${escapeHtml(t.description)}</div>`
                      : ""
                  }
                  ${
                    (t.linkedRequirements || []).length ||
                    (t.linkedAcceptanceCriteria || []).length
                      ? `<div class="detail-task-links">
                          ${(t.linkedRequirements || [])
                            .map((r) => `<span class="chip blue">${escapeHtml(r)}</span>`)
                            .join("")}
                          ${(t.linkedAcceptanceCriteria || [])
                            .map((a) => `<span class="chip slate">${escapeHtml(a)}</span>`)
                            .join("")}
                        </div>`
                      : ""
                  }
                </div>
              </li>
            `
              )
              .join("")}
          </ul>
        </div>
      `);
    }

    if (!phaseBlocks.length) continue;

    totalVisible += versionTaskCount;

    const section = document.createElement("div");
    section.className = "detail-version" + (isCurrent ? " current" : "");
    section.innerHTML = `
      <div class="detail-version-head">
        <span class="version-id">
          <span class="chip ${isCurrent ? "accent" : "slate"}">${isCurrent ? "current" : "past"}</span>
          ${escapeHtml(version.id)}
        </span>
        <span class="detail-version-counts">
          <span class="chip green">${s.completed} done</span>
          <span class="chip amber">${s.pending} pending</span>
          <span class="chip slate">${s.percent}%</span>
        </span>
      </div>
      ${phaseBlocks.join("")}
    `;
    frag.appendChild(section);
    renderedVersions++;
  }

  container.innerHTML = "";
  if (renderedVersions === 0) {
    container.innerHTML = `<div class="panel-loading">No tasks match the current filters.</div>`;
    setDetailTabCount("tasks", 0);
    return;
  }
  container.appendChild(frag);
  setDetailTabCount("tasks", totalVisible);
}

function setupFeatureDetail() {
  const back = document.getElementById("detail-back");
  if (back) {
    back.addEventListener("click", () => {
      window.history.pushState({ route: "features" }, "", "/features");
      showRoute("features");
    });
  }

  [
    { id: "detail-search", event: "input" },
    { id: "detail-status", event: "change" },
    { id: "detail-version", event: "change" },
    { id: "detail-phase", event: "change" }
  ].forEach(({ id, event }) => {
    const el = document.getElementById(id);
    if (el) el.addEventListener(event, renderDetailBody);
  });

  document.querySelectorAll("[data-detail-tab]").forEach((tab) => {
    tab.addEventListener("click", () => showDetailTab(tab.dataset.detailTab));
  });
}

/* Switch between the Tasks, Reports, and Spec Files panels on the feature
   detail page. Reports used to sit below the task list, which meant scrolling
   past every task to reach them. */
function showDetailTab(name) {
  const target = ["evidence", "reports", "specs"].includes(name) ? name : "tasks";

  document.querySelectorAll("[data-detail-tab]").forEach((tab) => {
    const active = tab.dataset.detailTab === target;
    tab.classList.toggle("active", active);
    tab.setAttribute("aria-selected", active ? "true" : "false");
  });

  document.querySelectorAll(".detail-panel").forEach((panel) => {
    panel.classList.toggle("active", panel.id === `detail-panel-${target}`);
  });
}

function setDetailTabCount(tabName, count) {
  const el = document.getElementById(`detail-tab-${tabName}-count`);
  if (!el) return;
  el.textContent = count > 0 ? String(count) : "";
  el.classList.toggle("empty", !(count > 0));
}

/* ============================================================
   TEAM & BRANCH
   ============================================================ */
async function renderTeam() {
  const team = await fetchJson("/api/team");
  const container = document.getElementById("team-list");

  if (!team || !team.members || team.members.length === 0) {
    container.className = "panel-loading";
    container.textContent = "No team members.";
  } else {
    container.className = "team-grid";
    container.innerHTML = "";

    team.members.forEach((m) => {
      const card = document.createElement("div");
      card.className = "member-card";
      card.innerHTML = `
        <div class="member-avatar role-${escapeHtml(m.role)}">${escapeHtml(initials(m.username))}</div>
        <div class="member-info">
          <div class="member-name">${escapeHtml(m.username)}</div>
          <div class="member-role">${escapeHtml(m.role)}</div>
        </div>
      `;
      container.appendChild(card);
    });

    const rules = team.rules || {};
    if (rules.approval_required || rules.implementation_allowed) {
      const rulesCard = document.createElement("div");
      rulesCard.className = "team-rules";
      rulesCard.innerHTML = `
        ${
          rules.approval_required
            ? `<div>
                <div class="rule-block-label">Approval Required</div>
                <div class="chip-list">${rules.approval_required
                  .map((r) => `<span class="chip green">${escapeHtml(r)}</span>`)
                  .join("")}</div>
              </div>`
            : ""
        }
        ${
          rules.implementation_allowed
            ? `<div>
                <div class="rule-block-label">Implementation Allowed</div>
                <div class="chip-list">${rules.implementation_allowed
                  .map((r) => `<span class="chip blue">${escapeHtml(r)}</span>`)
                  .join("")}</div>
              </div>`
            : ""
        }
      `;
      container.appendChild(rulesCard);
    }
  }

  await renderBranches();
}

async function renderBranches() {
  const container = document.getElementById("branch-list");
  if (!container) return;

  const data = await fetchJson("/api/branches");

  if (!data || !data.available || !data.branches || data.branches.length === 0) {
    container.className = "panel-loading";
    container.textContent = "No git branches found.";
    return;
  }

  container.className = "branch-list";
  container.innerHTML = "";

  data.branches.forEach((b) => {
    const row = document.createElement("div");
    row.className = "branch-row" + (b.current ? " is-current" : "");

    const statusChip = b.current
      ? `<span class="chip blue">current</span>`
      : b.merged
        ? `<span class="chip green">merged</span>`
        : `<span class="chip amber">active</span>`;

    row.innerHTML = `
      <div class="branch-main">
        <div class="branch-name">
          <span class="branch-icon">⎇</span>${escapeHtml(b.name)}
          ${statusChip}
        </div>
        <div class="branch-subject">${escapeHtml(b.subject || "—")}</div>
      </div>
      <div class="branch-meta">
        <span class="branch-commit">${escapeHtml(b.commit || "")}</span>
        <span class="branch-author">${escapeHtml(b.author || "")}</span>
        <span class="branch-date">${escapeHtml(formatDate(b.committedAt))}</span>
      </div>
    `;
    container.appendChild(row);
  });
}

/* ============================================================
   ACTIVITY
   ============================================================ */
function auditTone(event) {
  if (/approved|merged|completed/i.test(event)) return "green";
  if (/created|version/i.test(event)) return "blue";
  if (/rejected|failed|blocked/i.test(event)) return "amber";
  return "";
}

async function renderAudit() {
  const data = await fetchJson("/api/activity");
  const container = document.getElementById("audit-list");

  const events = (data && data.events) || [];

  if (events.length === 0) {
    container.className = "panel-loading";
    container.textContent = "No activity yet.";
    return;
  }

  const ACTIVITY_PER_PAGE = 20;

  // The timeline and the pager are siblings, so wrap them in a host element
  // that survives re-renders. The host replaces the timeline as the child.
  const host = document.createElement("div");
  host.className = "activity-host";

  const timeline = document.createElement("div");
  timeline.className = "timeline";

  const pagerHost = document.createElement("div");

  const timelineItem = (e) => {
    const item = document.createElement("div");
    item.className = "timeline-item";

    const details = e.details || {};
    const detailParts = Object.entries(details)
      .map(([k, v]) => `${k}: ${v}`)
      .join(" · ");

    const scope = [e.feature, e.version].filter(Boolean).join(" · ");

    // Actor attribution: older events predate actor capture, so fall back to a
    // muted "unknown" chip rather than hiding the row entirely.
    const actor = e.actor ? String(e.actor) : "";
    const actorChip = actor
      ? `<span class="timeline-actor" title="Performed by ${escapeHtml(actor)}">${escapeHtml(actor)}</span>`
      : `<span class="timeline-actor unknown" title="Actor not recorded for this event">unknown</span>`;

    const sourceLabel = e.source === "audit-log" ? "audit log" : "workflow";

    item.innerHTML = `
      <span class="timeline-dot ${auditTone(e.event)}"></span>
      <div class="timeline-body">
        <div class="timeline-event">${escapeHtml(String(e.event).replace(/[_-]/g, " "))}</div>
        <div class="timeline-meta">
          ${actorChip}
          <span class="timeline-time">${escapeHtml(formatDateTime(e.timestamp))}</span>
          ${scope ? `<span class="timeline-scope">${escapeHtml(scope)}</span>` : ""}
          <span class="timeline-source">${escapeHtml(sourceLabel)}</span>
        </div>
        ${detailParts ? `<div class="timeline-details">${escapeHtml(detailParts)}</div>` : ""}
      </div>
    `;
    return item;
  };

  const repaint = () => {
    const slice = paginate(events, "activity", ACTIVITY_PER_PAGE);
    timeline.innerHTML = "";
    slice.items.forEach((e) => timeline.appendChild(timelineItem(e)));

    pagerHost.innerHTML = "";
    const pager = renderPager(events.length, "activity", ACTIVITY_PER_PAGE, repaint);
    if (pager) pagerHost.appendChild(pager);
  };

  host.appendChild(timeline);
  host.appendChild(pagerHost);

  container.className = "";
  container.innerHTML = "";
  container.appendChild(host);

  repaint();
}

/* ============================================================
   SKILLS
   ============================================================ */
async function renderSkills() {
  const data = await fetchJson("/api/skills");
  const container = document.getElementById("skills-list");
  if (!container) return;

  const skills = (data && data.skills) || [];

  if (skills.length === 0) {
    container.className = "panel-loading";
    container.textContent = "No skills provisioned yet. Run /sdrive:skills to discover and install skills.";
    return;
  }

  const SKILLS_PER_PAGE = 9;

  // The grid and the pager are siblings, so wrap them in a host element that
  // survives re-renders. The host replaces the grid as the section's child.
  const host = document.createElement("div");
  host.className = "skills-host";

  const grid = document.createElement("div");
  grid.className = "skills-grid";

  const pagerHost = document.createElement("div");

  const skillCard = (s) => {
    const card = document.createElement("div");
    card.className = "skill-card";

    const tags = (s.tags || [])
      .map((t) => `<span class="chip slate">${escapeHtml(t)}</span>`)
      .join("");

    const meta = [
      s.version ? `v${s.version}` : "",
      s.installed ? `installed ${formatDate(s.installed)}` : "",
      s.lines ? `${s.lines} lines` : "",
      s.extraFiles ? `${s.extraFiles} extra file${s.extraFiles === 1 ? "" : "s"}` : ""
    ]
      .filter(Boolean)
      .join(" · ");

    card.innerHTML = `
      <div class="skill-head">
        ${
          s.logo
            ? `<span class="skill-logo"><img src="${escapeHtml(s.logo)}" alt="${escapeHtml(
                s.name
              )} logo" loading="lazy" /></span>`
            : `<span class="skill-icon">✦</span>`
        }
        <div class="skill-title">
          <div class="skill-name">${escapeHtml(s.name)}</div>
          <div class="skill-folder">.sdrive/skills/${escapeHtml(s.folder)}</div>
        </div>
        ${s.version ? `<span class="skill-version">v${escapeHtml(s.version)}</span>` : ""}
      </div>
      <div class="skill-desc">${escapeHtml(s.description || "No description.")}</div>
      ${tags ? `<div class="chip-list">${tags}</div>` : ""}
      <div class="skill-foot">
        <div class="skill-meta">${escapeHtml(meta)}</div>
        ${s.source ? `<div class="skill-source">${escapeHtml(s.source)}</div>` : ""}
      </div>
    `;
    return card;
  };

  const repaint = () => {
    const slice = paginate(skills, "skills", SKILLS_PER_PAGE);
    grid.innerHTML = "";
    slice.items.forEach((s) => grid.appendChild(skillCard(s)));

    pagerHost.innerHTML = "";
    const pager = renderPager(skills.length, "skills", SKILLS_PER_PAGE, repaint);
    if (pager) pagerHost.appendChild(pager);
  };

  host.appendChild(grid);
  host.appendChild(pagerHost);

  container.className = "";
  container.innerHTML = "";
  container.appendChild(host);

  repaint();
}

/* ============================================================
   COMMAND REFERENCE
   ============================================================ */
let commandData = { slash: [], cli: [] };
let activeCommandTab = "slash";
let commandQuery = "";

function commandMatches(item) {
  if (!commandQuery) return true;
  const haystack = `${item.command} ${item.description} ${item.group || ""} ${item.what || ""} ${item.why || ""} ${item.when || ""}`.toLowerCase();
  return haystack.includes(commandQuery);
}

function renderCommandList() {
  const container = document.getElementById("command-list");
  if (!container) return;

  const source = activeCommandTab === "slash" ? commandData.slash : commandData.cli;
  const items = source.filter(commandMatches);

  if (items.length === 0) {
    container.className = "command-list";
    container.innerHTML = `<div class="command-empty">No commands match “${escapeHtml(commandQuery)}”.</div>`;
    return;
  }

  container.className = "command-list";
  container.innerHTML = "";

  // Group CLI commands by their group; slash commands render as one table.
  if (activeCommandTab === "cli") {
    const groups = {};
    items.forEach((item) => {
      const g = item.group || "Other";
      (groups[g] = groups[g] || []).push(item);
    });

    Object.keys(groups).forEach((groupName) => {
      const label = document.createElement("div");
      label.className = "command-group-label";
      label.textContent = groupName;
      container.appendChild(label);
      container.appendChild(commandTableEl(groups[groupName]));
    });
  } else {
    container.appendChild(commandTableEl(items));
  }
}

function commandTableEl(items) {
  const wrap = document.createElement("div");
  wrap.className = "table-scroll";

  const table = document.createElement("table");
  table.className = "command-table";

  const head = document.createElement("thead");
  head.innerHTML = `
    <tr>
      <th class="col-command">Command</th>
      <th>What it does</th>
      <th class="col-meta">Details</th>
    </tr>
  `;
  table.appendChild(head);

  const body = document.createElement("tbody");
  items.forEach((item) => body.appendChild(commandRowEl(item)));
  table.appendChild(body);

  wrap.appendChild(table);
  return wrap;
}

// Returns a DocumentFragment holding the main row plus (when available) a
// hidden detail row that the "Details" button toggles.
function commandRowEl(item) {
  const frag = document.createDocumentFragment();
  const tr = document.createElement("tr");
  tr.className = "command-row";

  const meta = [];
  if (item.acceptsFeature) meta.push(`<span class="chip blue">feature</span>`);
  if (item.acceptsAll) meta.push(`<span class="chip green">--all</span>`);
  if (item.agent) meta.push(`<span class="chip slate">${escapeHtml(item.agent)}</span>`);
  (item.gates || []).forEach((g) => meta.push(`<span class="chip amber">gate: ${escapeHtml(g)}</span>`));
  if (item.optional) meta.push(`<span class="chip optional-chip" title="${escapeHtml(item.optionalNote || "")}">optional</span>`);

  const hasDetail = Boolean(
    item.what ||
      item.why ||
      item.when ||
      item.example ||
      (item.does || []).length ||
      (item.produces || []).length
  );
  const summary = item.what || item.description;

  tr.innerHTML = `
    <td class="col-command"><code class="command-code" title="Click to copy">${escapeHtml(item.command)}</code></td>
    <td>
      <div class="command-desc">${escapeHtml(summary)}</div>
      ${item.what && item.description ? `<div class="command-sub">${escapeHtml(item.description)}</div>` : ""}
    </td>
    <td class="col-meta">
      ${meta.length ? `<div class="command-meta">${meta.join("")}</div>` : ""}
      ${hasDetail ? `<button class="command-toggle" type="button" aria-expanded="false">Details</button>` : ""}
    </td>
  `;

  const code = tr.querySelector(".command-code");
  code.addEventListener("click", () => {
    const text = item.command;
    if (navigator.clipboard) {
      navigator.clipboard.writeText(text).then(() => {
        const original = code.textContent;
        code.textContent = "copied!";
        setTimeout(() => (code.textContent = original), 900);
      });
    }
  });

  if (hasDetail) {
    const toggle = tr.querySelector(".command-toggle");
    toggle.addEventListener("click", () => openCommandDrawer(item));
  }

  frag.appendChild(tr);
  return frag;
}

/* ============================================================
   COMMAND DETAIL DRAWER
   Slides in from the right with the full picture of a command:
   what it does, why it matters, when to run it, the concrete
   steps it performs, and the files it reads and writes.
   ============================================================ */
let commandDrawerReturnFocus = null;

function openCommandDrawer(item) {
  const drawer = document.getElementById("command-drawer");
  const backdrop = document.getElementById("command-drawer-backdrop");
  const title = document.getElementById("command-drawer-title");
  const body = document.getElementById("command-drawer-body");
  if (!drawer || !body) return;

  commandDrawerReturnFocus = document.activeElement;

  if (title) title.textContent = item.command;
  body.innerHTML = commandDrawerBody(item);

  drawer.classList.add("open");
  drawer.setAttribute("aria-hidden", "false");
  if (backdrop) backdrop.classList.add("open");
  document.body.classList.add("drawer-open");

  const close = document.getElementById("command-drawer-close");
  if (close) close.focus();
}

function closeCommandDrawer() {
  const drawer = document.getElementById("command-drawer");
  const backdrop = document.getElementById("command-drawer-backdrop");
  if (!drawer || !drawer.classList.contains("open")) return;

  drawer.classList.remove("open");
  drawer.setAttribute("aria-hidden", "true");
  if (backdrop) backdrop.classList.remove("open");
  document.body.classList.remove("drawer-open");

  if (commandDrawerReturnFocus && commandDrawerReturnFocus.focus) {
    commandDrawerReturnFocus.focus();
  }
  commandDrawerReturnFocus = null;
}

function commandDrawerBody(item) {
  const sections = [];

  const prose = [];
  if (item.what) prose.push(drawerProse("What it does", item.what));
  if (item.why) prose.push(drawerProse("Why it matters", item.why));
  if (item.when) prose.push(drawerProse("When to use it", item.when));
  if (prose.length) sections.push(`<div class="drawer-prose">${prose.join("")}</div>`);

  if ((item.does || []).length) {
    sections.push(`
      <section class="drawer-section">
        <h3 class="drawer-section-title">What it does, step by step</h3>
        <ol class="drawer-steps">
          ${item.does.map((d) => `<li>${escapeHtml(d)}</li>`).join("")}
        </ol>
      </section>
    `);
  }

  if ((item.produces || []).length) {
    sections.push(`
      <section class="drawer-section">
        <h3 class="drawer-section-title">Files &amp; artifacts it generates</h3>
        <ul class="drawer-paths">
          ${item.produces
            .map((p) => `<li><code class="drawer-path">${escapeHtml(p)}</code></li>`)
            .join("")}
        </ul>
      </section>
    `);
  }

  if ((item.reads || []).length) {
    sections.push(`
      <section class="drawer-section">
        <h3 class="drawer-section-title">What it reads</h3>
        <ul class="drawer-paths drawer-paths-read">
          ${item.reads
            .map((p) => `<li><code class="drawer-path">${escapeHtml(p)}</code></li>`)
            .join("")}
        </ul>
      </section>
    `);
  }

  const facts = [];
  if (item.agent) facts.push(drawerFact("Agent", item.agent));
  if (item.acceptsFeature) facts.push(drawerFact("Scope", "Single feature"));
  if (item.acceptsAll) facts.push(drawerFact("Batch", "Supports --all"));
  if ((item.gates || []).length) facts.push(drawerFact("Gates", item.gates.join(", ")));
  if (item.handsOff) facts.push(drawerFact("Hands off to", item.handsOff));
  if (facts.length) {
    sections.push(`
      <section class="drawer-section">
        <h3 class="drawer-section-title">At a glance</h3>
        <dl class="drawer-facts">${facts.join("")}</dl>
      </section>
    `);
  }

  if (item.optional && item.optionalNote) {
    sections.push(`
      <aside class="drawer-note">
        <span class="drawer-note-label">Optional</span>
        <p>${escapeHtml(item.optionalNote)}</p>
      </aside>
    `);
  }

  return sections.join("");
}

function drawerProse(label, value) {
  return `<div class="drawer-prose-item">
    <span class="drawer-prose-label">${escapeHtml(label)}</span>
    <p>${escapeHtml(value)}</p>
  </div>`;
}

function drawerFact(label, value) {
  return `<div class="drawer-fact">
    <dt>${escapeHtml(label)}</dt>
    <dd>${escapeHtml(value)}</dd>
  </div>`;
}

function setupCommandDrawer() {
  const close = document.getElementById("command-drawer-close");
  const backdrop = document.getElementById("command-drawer-backdrop");
  if (close) close.addEventListener("click", closeCommandDrawer);
  if (backdrop) backdrop.addEventListener("click", closeCommandDrawer);

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeCommandDrawer();
  });
}

function renderWorkflowCards() {
  const container = document.getElementById("workflow-cards");
  if (!container) return;

  const workflows = (commandData && commandData.workflows) || [];
  if (!workflows.length) {
    container.innerHTML = "";
    return;
  }

  container.innerHTML = workflows
    .map(
      (w) => `
    <article class="workflow-card" data-workflow="${escapeHtml(w.id)}">
      <div class="workflow-card-head">
        <span class="workflow-icon">${escapeHtml(w.icon || "◆")}</span>
        <div>
          <h3 class="workflow-name">${escapeHtml(w.name)}</h3>
          <span class="workflow-tagline">${escapeHtml(w.tagline || "")}</span>
        </div>
      </div>
      <p class="workflow-summary">${escapeHtml(w.summary || "")}</p>
      <ol class="workflow-steps">
        ${(w.steps || [])
          .map((s) => {
            const isOptional = (w.optional || []).includes(s);
            return `<li${isOptional ? ' class="optional"' : ""}><code>${escapeHtml(s)}</code>${
              isOptional ? '<span class="step-optional-badge">optional</span>' : ""
            }</li>`;
          })
          .join("")}
        ${(w.iteration || [])
          .map(
            (s) =>
              `<li class="workflow-iteration-step"><code>${escapeHtml(s)}</code></li>`
          )
          .join("")}
      </ol>
      ${
        w.optionalNote
          ? `<p class="workflow-note">${escapeHtml(w.optionalNote)}</p>`
          : ""
      }
      ${
        w.migrationNote
          ? `<aside class="workflow-subcard">
              <div class="workflow-subcard-head">
                <span class="workflow-subcard-icon">⇄</span>
                <h4 class="workflow-subcard-title">${escapeHtml(w.migrationNote.title)}</h4>
              </div>
              <p class="workflow-subcard-body">${escapeHtml(w.migrationNote.body)}</p>
            </aside>`
          : ""
      }
    </article>
  `
    )
    .join("");

  // Optional Commands card — a separate card below the lifecycle cards that
  // groups the genuinely on-demand commands (performance, docs, secure, test).
  // /sdrive:archive is deliberately NOT here: it is the compulsory final step of
  // the core lifecycle, so it belongs to the workflow cards above.
  // It spans the full width of the grid row.
  const optional = commandData && commandData.optionalCommands;
  if (optional && (optional.commands || []).length) {
    const card = document.createElement("article");
    card.className = "workflow-card optional-commands-card";
    card.innerHTML = `
      <div class="optional-head">
        <span class="optional-icon">${escapeHtml(optional.icon || "✦")}</span>
        <div class="optional-head-text">
          <div class="optional-eyebrow">On demand</div>
          <h3 class="optional-title">${escapeHtml(optional.title || "Optional Commands")}</h3>
          <p class="optional-summary">${escapeHtml(optional.summary || "")}</p>
        </div>
        <span class="optional-tagline">${escapeHtml(optional.tagline || "")}</span>
      </div>
      <ul class="optional-command-list">
        ${(optional.commands || [])
          .map(
            (c) => `
          <li class="optional-command">
            <div class="optional-command-top">
              <code class="optional-command-code">${escapeHtml(c.command)}</code>
            </div>
            <div class="optional-command-body">
              <div class="optional-command-what">${escapeHtml(c.what || "")}</div>
              <div class="optional-command-when">
                <span class="optional-when-label">When</span>
                <span class="optional-when-text">${escapeHtml(c.when || "")}</span>
              </div>
            </div>
          </li>
        `
          )
          .join("")}
      </ul>
    `;
    container.appendChild(card);
  }
}

async function renderCommands() {
  const data = await fetchJson("/api/commands");
  commandData = data || { slash: [], cli: [], workflows: [] };

  const search = document.getElementById("command-search");
  if (search) {
    search.addEventListener("input", () => {
      commandQuery = search.value.trim().toLowerCase();
      renderCommandList();
    });
  }

  document.querySelectorAll(".command-tab").forEach((tab) => {
    tab.addEventListener("click", () => {
      document.querySelectorAll(".command-tab").forEach((t) => t.classList.remove("active"));
      tab.classList.add("active");
      activeCommandTab = tab.dataset.tab;
      renderCommandList();
    });
  });

  renderCommandList();
  renderWorkflowCards();
  setupCommandDrawer();
}

/* ============================================================
   ROUTER (path-based)
   Each view is its own URL: /overview, /product-plan, /features,
   /team, /activity, /commands. The server serves the same shell
   for every route; here we show the matching section and keep the
   sidebar highlight in sync.

   /features/<name> is a dynamic route that renders the feature
   detail page (task breakdown + filters) for a single feature.
   ============================================================ */
const ROUTES = [
  "overview",
  "product-plan",
  "features",
  "feature-detail",
  "quality",
  "skills",
  "team",
  "activity",
  "commands"
];
const DEFAULT_ROUTE = "overview";

// Returns { route, feature } for the current URL. A path like
// /features/about-page resolves to { route: "feature-detail", feature: "about-page" }.
function parseLocation() {
  const path = window.location.pathname.replace(/\/+$/, "");
  const parts = path.split("/").filter(Boolean);

  if (parts[0] === "features" && parts.length > 1) {
    return { route: "feature-detail", feature: decodeURIComponent(parts[1]) };
  }

  const name = parts[parts.length - 1] || "";
  return { route: ROUTES.includes(name) ? name : DEFAULT_ROUTE, feature: null };
}

function currentRoute() {
  return parseLocation().route;
}

function showRoute(route) {
  const target = ROUTES.includes(route) ? route : DEFAULT_ROUTE;

  document.querySelectorAll(".section").forEach((section) => {
    section.classList.toggle("active", section.id === target);
  });

  // The sidebar highlights "Features" while viewing a feature detail page.
  const navTarget = target === "feature-detail" ? "features" : target;
  document.querySelectorAll(".nav-item").forEach((item) => {
    item.classList.toggle("active", item.dataset.nav === navTarget);
  });

  const label = target === "feature-detail" ? "Feature" : target;
  document.title = `SpecDrive · ${label.charAt(0).toUpperCase()}${label.slice(1)}`;
  window.scrollTo({ top: 0, behavior: "auto" });

  if (target === "quality") renderQuality();
}

function setupNav() {
  // Intercept sidebar link clicks so navigation is instant (no full reload)
  // while still updating the URL via the History API.
  document.querySelectorAll(".nav-item").forEach((item) => {
    item.addEventListener("click", (event) => {
      const route = item.dataset.nav;
      if (!ROUTES.includes(route)) return;
      event.preventDefault();
      if (currentRoute() !== route) {
        window.history.pushState({ route }, "", `/${route}`);
      }
      showRoute(route);
      closeSidebarDrawer();
    });
  });

  // Handle browser back/forward.
  window.addEventListener("popstate", () => {
    const { route, feature } = parseLocation();
    showRoute(route);
    if (route === "feature-detail" && feature) openFeatureDetail(feature);
  });

  showRoute(currentRoute());
}

/* Mobile off-canvas drawer (shadcn-style sheet). */
function openSidebarDrawer() {
  const sidebar = document.querySelector(".sidebar");
  const backdrop = document.getElementById("sidebar-backdrop");
  const toggle = document.getElementById("sidebar-toggle");
  if (sidebar) sidebar.classList.add("open");
  if (backdrop) backdrop.classList.add("open");
  if (toggle) toggle.setAttribute("aria-expanded", "true");
}

function closeSidebarDrawer() {
  const sidebar = document.querySelector(".sidebar");
  const backdrop = document.getElementById("sidebar-backdrop");
  const toggle = document.getElementById("sidebar-toggle");
  if (sidebar) sidebar.classList.remove("open");
  if (backdrop) backdrop.classList.remove("open");
  if (toggle) toggle.setAttribute("aria-expanded", "false");
}

function setupSidebarDrawer() {
  const toggle = document.getElementById("sidebar-toggle");
  const backdrop = document.getElementById("sidebar-backdrop");
  if (toggle) {
    toggle.addEventListener("click", () => {
      const sidebar = document.querySelector(".sidebar");
      if (sidebar && sidebar.classList.contains("open")) {
        closeSidebarDrawer();
      } else {
        openSidebarDrawer();
      }
    });
  }
  if (backdrop) backdrop.addEventListener("click", closeSidebarDrawer);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") closeSidebarDrawer();
  });
}

/* ============================================================
   THEME
   ============================================================ */

const THEME_KEY = "sdrive-theme";

/* An inline script in index.html sets <html data-theme> before first paint, so
   this never has to apply the stored theme - it only reacts to clicks and keeps
   the button's own label in step. The stored value is what that script reads. */
function applyTheme(theme) {
  const next = theme === "dark" ? "dark" : "light";
  document.documentElement.setAttribute("data-theme", next);

  try {
    window.localStorage.setItem(THEME_KEY, next);
  } catch (error) {
    /* localStorage can be blocked (private mode). The toggle still works for this session. */
  }

  syncThemeToggle(next);
}

/* The button advertises the theme it switches TO, not the one you are in, so the
   label always describes the action a click will take. */
function syncThemeToggle(theme) {
  const current = theme || document.documentElement.getAttribute("data-theme") || "dark";
  const button = document.getElementById("theme-toggle");
  if (!button) return;

  const icon = document.getElementById("theme-toggle-icon");
  const label = document.getElementById("theme-toggle-label");
  const nextName = current === "dark" ? "light" : "dark";

  button.setAttribute("aria-label", `Switch to ${nextName} theme`);
  button.setAttribute("title", `Switch to ${nextName} theme`);
  button.setAttribute("aria-pressed", current === "dark" ? "true" : "false");
  if (label) label.textContent = nextName === "dark" ? "Dark" : "Light";
  if (icon) icon.textContent = current === "dark" ? "☀" : "☾";
}

function setupThemeToggle() {
  const button = document.getElementById("theme-toggle");
  if (button) {
    button.addEventListener("click", () => {
      const current = document.documentElement.getAttribute("data-theme") || "dark";
      applyTheme(current === "dark" ? "light" : "dark");
    });
  }

  // Keep in step when the theme changes in another tab. The OS preference is
  // intentionally not watched: dark is the default, and an explicit choice wins.
  window.addEventListener("storage", (event) => {
    if (event.key === THEME_KEY && (event.newValue === "light" || event.newValue === "dark")) {
      document.documentElement.setAttribute("data-theme", event.newValue);
      syncThemeToggle(event.newValue);
    }
  });

  syncThemeToggle();
}

/* ============================================================
   MAIN
   ============================================================ */

/* The sticky panels inside a page (task filters, Spec Files header and search
   bar) park themselves under the topbar using `top: var(--topbar-h)`. That
   variable is a static guess in CSS, but the topbar's real height changes with
   its content — the project name and git chip wrap onto extra lines at narrow
   widths, and a long branch name can push it taller still. When the guess is
   short, the sticky panels park too high and page content scrolls through the
   gap between them and the topbar.

   Measuring the topbar and writing the value back to the CSS variable keeps
   every sticky offset exact at any width. A ResizeObserver re-measures whenever
   the topbar's box changes (wrap, font load, project name swap). */
function syncTopbarHeight() {
  const topbar = document.querySelector(".topbar");
  if (!topbar) return;

  const apply = () => {
    const height = Math.round(topbar.getBoundingClientRect().height);
    if (height > 0) {
      document.documentElement.style.setProperty("--topbar-h", `${height}px`);
    }
  };

  apply();

  if (typeof ResizeObserver === "function") {
    new ResizeObserver(apply).observe(topbar);
  } else {
    window.addEventListener("resize", apply);
  }
}

// Fetch everything the shell needs and paint it. Kept separate from main() so the
// live refresh can re-run exactly this without re-binding any events.
//
// Each panel paints as its own payload lands. This used to be a single Promise.all, so
// the slowest endpoint held every panel on "Loading…" even though the other five had
// already answered - and /api/git-info shells out to git a dozen times, which is seconds
// on a machine where git itself is slow. A failed request now leaves its own panel
// untouched rather than replacing it with a claim the server never made.
async function loadAndRenderShell() {
  const data = { onboarding: null, product: null, git: null, project: null, productType: null, health: null };
  const loaded = new Set();

  const paintHeader = () => {
    if (loaded.has("project")) renderProjectInfo(data.project);
    if (loaded.has("git")) renderGitInfo(data.git);
  };

  const paintOverview = () => {
    const container = document.getElementById("overview-panel");
    if (!container) return;
    if (!["onboarding", "product", "git", "health"].some((key) => loaded.has(key))) return;
    container.innerHTML = "";
    container.appendChild(renderOverview(data.onboarding, data.git, data.product, data.productType, data.health));
  };

  const paintProduct = () => {
    // Nothing to say until the plan has actually been read: rendering the "no plan"
    // empty state for a failed request would state something the server never said.
    if (!loaded.has("product")) return;
    const container = document.getElementById("product-panel");
    if (!container) return;
    container.innerHTML = "";
    container.appendChild(renderProduct(data.product, data.productType));
  };

  const jobs = [
    ["onboarding", "/api/onboarding", paintOverview],
    ["product", "/api/product", () => { paintProduct(); paintOverview(); }],
    ["git", "/api/git-info", () => { paintHeader(); paintOverview(); }],
    ["project", "/api/project-info", paintHeader],
    ["productType", "/api/product-type", () => { paintProduct(); paintOverview(); }],
    ["health", "/api/health", paintOverview]
  ];

  await Promise.all(
    jobs.map(async ([key, endpoint, afterPaint]) => {
      try {
        data[key] = await fetchJson(endpoint);
        loaded.add(key);
      } catch (error) {
        console.warn(`Could not load ${endpoint}: ${error.message}`);
      } finally {
        if (loaded.has(key)) afterPaint();
      }
    })
  );
}

// The dashboard used to show stale data until a manual refresh. Polling the real
// APIs on a timer would be worse than the bug it fixes: it would repaint while
// someone is reading, and reset state they had set. So poll a cheap fingerprint
// and refresh only when it genuinely changes.
const LIVE_POLL_MS = 5000;

function setupLiveRefresh(refresh) {
  let last = null;
  let running = false;

  const tick = async () => {
    // Skip while a refresh is in flight, and while the tab is hidden - there is
    // nothing to keep current, and the interval would just burn requests.
    if (running || document.hidden) return;
    running = true;
    try {
      const { stamp } = await fetchJson("/api/stamp");
      if (last === null) {
        last = stamp;
      } else if (stamp !== last) {
        last = stamp;
        await refresh();
      }
    } catch (error) {
      // A failed poll is not worth interrupting anyone for; the next one retries.
    } finally {
      running = false;
    }
  };

  setInterval(tick, LIVE_POLL_MS);

  // Returning to a hidden tab should catch up immediately rather than waiting out
  // the rest of the interval.
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) tick();
  });
}

async function main() {
  // Wire the theme toggle before the data fetches below. It needs no fetched
  // data, and running it first means the button shows the right label straight
  // away instead of the static "Theme" placeholder until the panels resolve.
  setupThemeToggle();
  setupProjectSwitcher();

  await loadAndRenderShell();
  await renderProjectSwitcher();

  // Bind each control to the single event it actually emits. A <select> fires
  // BOTH `input` and `change`, so binding both caused renderFeatures() to run
  // twice per change and race against itself (duplicate cards). Text inputs only
  // emit `input`; selects and checkboxes only need `change`.
  [
    { id: "filter-search", event: "input" },
    { id: "filter-phase", event: "change" },
    { id: "filter-verification", event: "change" },
    { id: "filter-tests", event: "change" },
    { id: "filter-current-version", event: "change" }
  ].forEach(({ id, event }) => {
    const el = document.getElementById(id);
    if (el) {
      el.addEventListener(event, renderFeatures);
    }
  });

  setupNav();
  setupSidebarDrawer();
  setupFeatureDetail();
  syncTopbarHeight();

  await renderFeatures();
  await renderSkills();
  await renderTeam();
  await renderAudit();
  await renderCommands();

  // Keep the page current without a manual refresh. Only the panels that read
  // .sdrive/ state are re-run; event bindings and the sidebar are left alone, so
  // filters and the current page survive a refresh.
  setupLiveRefresh(async () => {
    await loadAndRenderShell();
    await renderFeatures();
    await renderTeam();
    await renderAudit();
  });

  // Deep link: /features/<name> loads the detail page directly.
  const { route, feature } = parseLocation();
  if (route === "feature-detail" && feature) {
    await openFeatureDetail(feature);
  }
}

main();
