# Taskmaster ⚡

Project management and workflow orchestration platform built with [Dioxus 0.7](https://dioxuslabs.com), [PostgreSQL](https://www.postgresql.org/), and [SQLx](https://github.com/launchbadge/sqlx). An experiment in full stack Rust.

Taskmaster bridges flexible organizational hierarchies with dynamic, fully customizable task workflows—offering multi-view projections (Kanban, Swimlanes, Table List, and Gantt timelines) alongside fine-grained controls implementing a system
capable of Principle-of-Least-Privilege using bitmaps.
In organizations, privileges are governed
by roles set by the organization governor, implementing a form of 
Role-Based-Access-Control (RBAC), although without privileges based on inheritance
of roles. The role capability inheritance may be future work.

---

## 🌟 Key Features in Current Stage

### 1. 🏢 Organizations & Governance Hierarchy
- **Multi-Tenant Organizations**: Group teams, departments, or guilds under structured organizations.
- **Granular Role-Based Access Control (RBAC)**: Custom organization roles backed by 64-bit permission bitmasks (`READ`, `SUGGEST`, `EXECUTE`, `ASSIGN`, `MANAGE_MEMBERS`, `ADMIN`, `MANAGE_WORKFLOW`, `MODERATE_DISCUSSIONS`).
- **Project Affiliation & Access Rules**: Affiliate projects with organizations to inherit access rules, member ceilings, and governance transfer mechanics.
- **Invitations & Member Directory**: Invite members by username or directory, assign custom roles, and track pending approvals.

### 2. 🔀 Dynamic Workflow Engine & Custom Columns
- **Custom Project Statuses**: Projects define their own statuses with custom names, categories (`Todo`, `InProgress`, `Review`, `Done`), badge accents, and execution order.
- **Semantic Workflow Rules**:
  - `is_initial`: Designates entry statuses for newly suggested tasks.
  - `allows_assignment`: Blocks premature assignment in preliminary states (e.g., Backlog / Triage).
  - `requires_assignee`: Enforces assignment requirements before tasks can transition into active execution states (e.g., In Progress).
- **Soft WIP Limits**: Set Work-In-Progress limits per column with real-time visual overload warnings while preserving write throughput.
- **Built-in Starter Templates**: Instantiate projects with proven workflow models:
  - **Classic Kanban**: Backlog &rarr; In Progress (WIP: 5) &rarr; Review (WIP: 3) &rarr; Done.
  - **Guild Raid Prep**: Roster &rarr; Attunement &rarr; Consumables &rarr; Raid Ready.
  - **Software Sprint**: Sprint Backlog &rarr; In Dev &rarr; Code Review &rarr; QA &rarr; Deployed.
  - **Legacy Flow**: Suggested &rarr; Accepted &rarr; Assigned &rarr; In Progress &rarr; Finished.
- **Safe Column Deletion & Remapping**: `ON DELETE RESTRICT` database protection coupled with an interactive task-remapping modal prevents orphaned tasks.

### 3. 📊 Pluggable Multi-View Projections
Switch between 4 real-time views on any project board with persistent client view preferences:
- **Classic Kanban Board**: Horizontal drag-friendly board with column selector filters and WIP limit alerts.
- **Assignee Swimlanes**: Horizontal swimlane groupings per team member plus an unassigned pool.
- **Table List View**: Compact, tabular task ledger with status badges, assignees, and quick status actions.
- **Gantt Chart Timeline**: Visual project roadmap rendering scheduled tasks, date bounds, and completion percentages.

### 4. 📋 Task Lifecycle & Collaboration
- **Quick Creation**: Inline modal creation directly on project boards or via the global "My Tasks" view.
- **Aggregated "My Tasks" Dashboard**: Unified view consolidating all tasks assigned to or authored by the logged-in user across all affiliated projects.
- **Task Detail View**: Edit titles, rich descriptions, dates, estimated hours, and progress percentages.
- **Discussion Stream & Audit Log**: Real-time comment thread per task documenting collaboration and operational decisions.

---

## 🚀 Quick Start

### Prerequisites
- [Rust](https://rustup.rs/) (1.80+)
- [Docker](https://www.docker.com/) & Docker Compose
- [Dioxus CLI](https://dioxuslabs.com/learn/0.7/getting_started) (`cargo install dioxus-cli --version 0.7.1`)

### 1. Launch with Docker & Launch Script
The project includes an automated `./launch.sh` runner that manages database containers, environment variables, healthchecks, and application builds:

```bash
# Launch PostgreSQL container and start the Dioxus fullstack development server
./launch.sh up
```
Open **[http://127.0.0.1:8080](http://127.0.0.1:8080)** in your browser.

### 2. Standalone Development (dx serve)
If you already have PostgreSQL running (or started via `./launch.sh db`):

```bash
# 1. Start database container only
./launch.sh db

# 2. Start interactive Dioxus hot-reloading development server
dx serve --platform web --fullstack
```

---

## 🧭 How to Use Taskmaster

### 1. Register and Authenticate
- Navigate to `/register` to create a user account.
- The authentication layer provides persistent session tokens with automatic client hydration and secure password hashing.

### 2. Create Your First Project & Choose a Workflow
- Go to the **Dashboard** (`/dashboard`) and click **+ New Project**.
- Give your project a title, optional description, and select a starter template (e.g., *Classic Kanban* or *Software Sprint*).
- Click into your new project to view the board.

### 3. Manage Project Workflow Columns
- In the project board top bar, click **Manage Project** (`/projects/:id/manage`).
- Navigate to the **Workflow Columns** section to:
  - Add custom stages with customized badge colors and WIP limits.
  - Reorder stages using **▲ / ▼** controls.
  - Toggle semantic requirements like `Requires Assignee` or `Allows Assignment`.
  - Delete unused columns with safe task remapping.

### 4. Add Tasks & Switch Views
- Click **+ New Task** on the project board to add tasks to the initial stage.
- Switch between views at the top of the board:
  - **Kanban Classic** for standard stage-by-stage progression.
  - **Swimlanes** to inspect workload distribution across assignees.
  - **List View** for high-density scanning and bulk operations.
  - **Gantt Chart** to track schedules and completion progress.

### 5. Collaborate in Organizations
- Create an organization on the dashboard to group projects and collaborators.
- Define custom roles (e.g., *Project Lead*, *Contributor*, *Auditor*) with custom permission masks under `/organizations/:id/roles`.
- Invite members and affiliate your projects.

---

## 🧪 Testing & Verification

Taskmaster includes a comprehensive suite of 59 integration tests, schema-isolated database validations, SSR snapshot tests, and view render assertions:

```bash
# Run all tests (starts isolated PostgreSQL container and executes test suite)
./launch.sh --test-all

# Run backend integration tests only
./launch.sh --test-backend

# Run frontend Server-Side Rendering (SSR) smoke tests
./launch.sh --test-frontend

# Run tests with a specific thread concurrency
./launch.sh --test-all --threads 4
```

---

## 🛠 Project Architecture

```
todolist/
├── migrations/                  # Sequential PostgreSQL schema migrations
│   ├── 0001_initial_schema.sql
│   ├── ...
│   └── 0007_workflow_contract_drop_legacy_status.sql
├── src/
│   ├── backend/
│   │   ├── app_endpoints/       # Dioxus #[server] RPC endpoints (Auth, Tasks, Orgs, Projects)
│   │   ├── database.rs          # SQLx connection pooling & isolated test runner schemas
│   │   └── middleware/          # Session authentication & rate limiting
│   ├── components/
│   │   ├── dashboard/           # User dashboard & project listings
│   │   ├── organizations/       # Organization directory, governance & roles
│   │   ├── projects/            # Project settings, members & workflow column editor
│   │   ├── tasks/               # Kanban column, card, modals & task view
│   │   ├── views/               # Multi-view projections (Kanban, Swimlanes, List, Gantt)
│   │   └── navbar.rs            # Top navigation bar
│   ├── types.rs                 # Shared domain types, bitmasks, templates & DTOs
│   └── main.rs                  # Axum server bootstrap & Dioxus application router
├── launch.sh                    # Container orchestration and automated test runner
└── Cargo.toml                   # Dioxus 0.7 fullstack dependency manifest
```

---

## 📄 License
GNU Affero AGPL-3.0 license.
