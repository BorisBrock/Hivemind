# Workzone Project Plane

Workzone is a GUI for managing AI coding agents across multiple projects.

## Concept

The user can define one or more projects, which are defined by a name, a location (disk folder) and a color. For each project, the user can create tickets in a beautiful markdown editor. When a ticket is ready, the user can hand it over to an AI coding agent (e.g. Claude Code or OpenCode). The coding agent is run in the background and handles whatever is in the ticket. During this process the ticket moves across a Kanban board with these columns:

- Planning: User works on the ticket. Nothing happens until user sends it to Ready.
- Ready: User is done with creating ticket. AI agent may pick it up to work on it.
- In progress: AI Agent is working on the ticket.
- Needs Input: AI agent has feedback or needs a decision from the user.
- Done: AI agent has finished work on the ticket. User can either close it or send it back to Ready with a comment (e.g. request a change).

This way the user can work on multiple projects and tasks at once, without loosing the overview.
So this tool replaces dozens of open terminal sessions running Claude Code.

## Tickets

Tickets have a title and a body. The title is for display only, the body is actually the input prompt for the AI agent.
The user writes tickets in plain text/markdown.

Once a ticket has been submitted by the user, the AI agent starts to work on it. Now the text cannot be edited anymore. Instead the conversation/thought process of the coding agent is shown in the ticket. So the ticket is basically like a terminal CLI with Claude Code running in it.

## GUI

The GUI looks modern and supports themes.

On the left hand side, there is a column in which all projects are listed.
The rest of the window is used for a Kanban board.

The projects show up as cards/items with a title (big) and the location/folder (small). There are also buttons to edit the project or to hide/show the tickets of this project in the kanban board. Also each project has a unique color used for its card and tickets. An inicator on each project shows if it has currently any activity or not.

Tickets appear in the Kanban board in the color of the project they belong to. They show a shortened overview of whats in it (or the tickets title). Clicking on a ticket opens it up in a big view, where the user can see the initial prompt + whole conversation/actions of the AI agent. Depending on the state it has buttons to cancel it (if in progress) or to move it (e.g. Planning -> Ready).

## AI

For each task a single conversation/session with the coding agent is used. This run in the background/headless mode, so the user does not see it. Only the ticket shows the inputs/outputs of the agent. If the AI agent requires input from the user (a choice or additional input), the user can enter it in the ticket.

For each project, only one ticket may be processed at a time, so they don't interfere.

## Settings

There is a settings dialog where the user can:
- Choose an application theme.
- Choose a color palette for tickets.
- Choose the AI agent (Claude Code, OpenCode etc.).

## Status bar

The tool shall also have a status bar showing currents stats:
- Number of active agent sessions currently running.
- % token usage per session and week (see Claude Codes usage limits).

## Tech Stack

I want a modern tech stack. It shall be fast, and allow for a modern, themed and snappy user interface.
That could be rust, tauri, iced etc. WHatever is best suited.