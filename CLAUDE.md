# TickTapDrum: repository guide

TickTapDrum is a drum machine for people with little music knowledge. The design is in
[`docs/DESIGN.md`](docs/DESIGN.md); its "Proof-of-concept milestones" section says what has been
done so far. How to build and run it is in [`BUILDING.md`](BUILDING.md).
[`README.md`](README.md) is for people who use the app, so keep build details out of it.

## How the user wants the work done

- **Do not start implementing until the user asks.** Discussing, researching and writing up
  designs are fine; code waits for an explicit go-ahead, usually "start the next milestone".
- **Act as the coordinator.** Split work into independent pieces and delegate them to subagents,
  at most 3 at a time. Keep integration, review, testing and commits in the main session. Each
  subagent that writes code works in its own crate or folder (or its own worktree) so no two write
  the same files, and does not commit.
- **Commit as you go**, one focused commit per piece. Push and merge only when the user asks.
- **Check on a real target before calling something done**, and say plainly what was not checked
  (for example, sound nobody has listened to). Record findings in `docs/DESIGN.md`.

## Worktrees

**Always work in a git worktree.** Before the first edit to a tracked file, call `EnterWorktree`
instead of `git checkout -b`. Never edit the main checkout directly. Worktrees live under
`.claude/worktrees/`, which git ignores.

## Pull requests

**`master` is protected: changes reach it only through a pull request**, and CI must pass first.
Push the worktree's branch and open a pull request with `gh pr create`; never merge into `master`
locally or push to it. Open the pull request when the user asks to push, and merge it only when the
user asks.

## Nothing committed describes the local machine

No tracked file may name a local drive or folder, a LAN address, personal hardware, or a person's
private details. That covers prose, comments, test data and images. Such facts go in
`CLAUDE.local.md` at the repository root, which git ignores. A committed file that needs one says
"see the local notes" or uses an invented sample such as `<your projects folder>`. The one
exception is the README's "Author" section, which names the author and their email at the user's
request.
