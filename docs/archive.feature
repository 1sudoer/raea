Feature: Archive: projects move to the account's raea cloud storage when the device needs the space
  Shane, 11-17, while his Mac's disk was full and his Documents went to R2 by hand: "this is a good instance where pro
  and team users should have their data archived as needed or automatically when you archive or when reaching full
  device and need to clear space the agent has a command to utilize it or migrate to it".

  Status 11-17: planned, nothing built. What was done by hand that day is the shape to follow:
  ~/Documents/raea-r2/archive.sh copied each folder, checked every file against the copy by size and hash, and
  deleted the local folder only after a clean check (139.9 GB, 27 projects).

  What it builds on, rather than beside:
    - The account's cloud storage: Pro 250 GB, Team 2 TB (apps/site/api/src/billing/plans.ts). Blobs are kept by
      sha256 (raea-sync/<user>/blobs/<sha256>) and reached through Git LFS (lfs.ts), so a file already synced costs
      no upload.
    - The feed's storage flow: storage_plan as a dry run, the person confirming in Settings › Storage, then the Trash
      with Put Back. Archiving is the paid-plan path beside the Trash: the space comes back and nothing is lost.
    - Agent keys with files:write (agent-keys.ts), so an agent working for the person can archive within its scopes.

  Rule: What archiving is

    Scenario: AR1. A project archived keeps its place
      Given a project in the workspace on a Pro or Team account
      When it is archived
      Then each of its files is in the account's storage, checked by size and sha256 against the local file
      And only after every file checks out are the local copies deleted
      And the folder keeps a small stub (the project's own files under a size limit, its feed entries and a manifest)
        so it is still listed in the app, marked "In raea cloud", with its cover and its last renders' posters
      And a failed upload or any file that differs stops the archive with nothing deleted

    Scenario: AR2. Restoring
      Given an archived project
      When the person opens it, or runs `raea archive restore NAME`, or an agent's command needs one of its files
      Then its files come back from storage, each checked by hash, with their modes, times and links
      And the copy in storage stays until the person deletes it there
      And any device signed in to the account can restore it, not only the one that archived it

    Scenario: AR3. What never moves on its own
      Then the project open in the app, a project any running render, game or agent has files open in, and a project
        changed in the last 7 days are never archived automatically
      And files a tool reads by path (a bench's scenes, a test fixture) are found the way 11-17's hand archive found
        them, and their project is kept unless the person chooses it

  Rule: When it happens

    Scenario: AR4. By hand
      When the person archives a project from its menu, or several from Settings › Storage
      Then they see each project's size, when it was last opened, and how much of their storage it would use
      And the archive runs in the background with its progress shown, and the space returns project by project

    Scenario: AR5. When the device runs low
      Given free space under 10 GB, or under 5% of the disk
      Then the app says so and offers a plan: the least recently opened projects first, superseded renders, and the
        caches raea can rebuild, with the space each frees
      And with "Archive automatically" on (Settings › Storage, Pro and Team), it archives the plan without asking,
        tells the person what moved, and offers to restore each project
      And a render or build that is about to run out of space is paused with that plan offered, not left to fail

    Scenario: AR6. When the cloud is full
      Given an archive that would pass the plan's storage
      Then it is refused before anything uploads, with what fits, what doesn't, and the plan that would hold it
      And Free accounts are offered an archive to a drive or a folder they choose, under the same checks

  Rule: Agents

    Scenario: AR7. An agent can plan and archive
      Then `raea archive plan` prints what would free space (JSON with --json), and the feed has the same as a tool
      And `raea archive NAME...` and `raea archive restore NAME...` do the work, reporting each project as it finishes
      And an agent archives only with the person's yes in that conversation, or with "Archive automatically" on
      And an agent that hits a full disk while working says so and proposes the plan, rather than deleting anything

  Rule: Checks

    Scenario: AR8. Before it ships
      Then a test archives and restores a project with a symlink, an empty folder, an executable, a 2 GB file and an
        LFS-tracked file, and finds every byte, mode, time and link as it was
      And a test cuts the network mid-upload and finds the project whole on the device and the archive resumable
      And a test fills a disk image to 95% and sees the plan offered, and with "Archive automatically" on, the space
        returned
