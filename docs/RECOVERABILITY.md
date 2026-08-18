# What "recoverable" means for something an agent wrote

**A write is recoverable when someone can obtain the value that was there before it, by a route
that still exists at the moment they ask.** That definition has three moving parts, and each one
fails independently: somebody has to have kept the old value, there has to be a way to read it
back, and the clock has to not have run out. A write that fails any one of the three is not
recoverable, whatever the tool's documentation calls it.

This page is the working definition, the measurements behind it, and the parts we do not know.
Everything here can be re-derived from the commands given at the bottom.

## Is undo the same thing as recoverable?

No, and conflating them is where most of the confusion sits.

**Undo** is an operation the tool offers. **Recoverable** is a property of the state after a write
lands. A tool can offer undo over a value nobody kept, in which case the undo needs the old value
as an argument and you do not have it. A service can retain an old value with no undo anywhere in
its interface, and it is still recoverable, by hand, through the history.

The useful question is not "does this have an undo button". It is **"if I ask an hour from now,
who has the old value, and can they give it to me".**

## What actually gets kept?

Read off the official API documentation of six services on 18 August 2026, across 27 kinds of
object: GitHub, Slack, Notion, Linear, Stripe, Google Workspace.

| | count | of 27 |
|:--|--:|--:|
| Kept, and readable through the API | **12** | measured 18 August 2026, across 27 object kinds |
| Kept where only a person can look, not a program | **5** | measured 18 August 2026, across 27 object kinds |
| Not kept at all | **10** | measured 18 August 2026, across 27 object kinds |

Each row repeats its own date and denominator on purpose. Rows get quoted one at a time, and a
row that says "same as above" stops being true the moment it travels.

The split is not arbitrary. Everything in the first row is **structure**: titles, labels,
assignees, states, file contents through git. Everything in the last row is **content**: issue
bodies, comments, pull request bodies, release notes, an edited message.

> **Services journal what changed. They do not journal what it was.**

That sentence is the whole finding, and it is why "the service probably kept it" is a reasonable
guess about a label and a bad guess about a paragraph.

## How much of a tool surface does this touch?

Measured against one widely deployed MCP server, `github/github-mcp-server`, at commit
`2198e8599bbb` on 18 August 2026, classified by tool name using published rules.

| | count | of 50 judgeable writes |
|:--|--:|--:|
| Touches something the service still has | **18** | measured 18 August 2026, of 50 judgeable write tools |
| Left undecided rather than rounded | **8** | measured 18 August 2026, of 50 judgeable write tools |
| Touches something nobody has once written | **24** | measured 18 August 2026, of 50 judgeable write tools |

Seven further tools fold create, update and delete into a single entry point, so their
reversibility cannot be read off the name. They are excluded from the denominator rather than
guessed at. Fifty nine tools only read and are not counted here at all.

**These two tables are separate measurements over different populations. Multiplying them is
not a valid operation.**

## Does recoverable expire?

Yes, and this is the part most often missed. Thirty days on one service, ninety on another, a
hundred and eighty for an audit log, thirty days or the last hundred revisions on another.

**Recoverable means recoverable now.** A value in the first row of the first table moves to the
last row when its window closes, without anything happening and without anyone being told.

## What can I select on afterwards?

Only what was captured at the moment of the write. This is the constraint that surprises people
latest and costs the most: if you want to later ask "undo everything this agent did that was
based on a stale read", something has to have recorded, at write time, that a read happened and
what it returned. Nothing reconstructs that afterwards.

**The set of questions you can ask later is fixed when the record is made, not when you ask.**

## Where this is wrong, in our favour

Stated because you would otherwise find it yourself, and both of these make our position look
better than it is.

- **Retention belongs to a tool and a field together, not to a tool.** A tool that writes both a
  title and a body sits in one row of the second table when it belongs in two.
- **The middle row of the first table is counted with the last row.** A person can still read
  those values through the interface. If you are willing to recover by hand rather than by
  program, that group belongs on the recoverable side, and the picture improves.

Two smaller ones. The 27 object kinds were selected by an outside pass, not defined by us, so
their representativeness is unverified. And the second table classifies by tool name only; no
arguments and no responses were inspected.

## Re-derive it

```
# the tool census, including the rules it applies
python3 tools/census/census.py --date 2026-08-18

# the figure, drawn from the census output rather than typed
python3 tools/census/figure.py
```

Method: `tools/census/rules.json`, which carries its own changelog and its own list of what it
does not look at. Object-level retention: recorded with sources in the distribution requirements
document, section 33. If you re-run the census after the upstream server changes, the numbers
will move, and the rules file records which version produced which count.

## Corrections

- **2026-08-18**: an earlier version of the tool classification filed renaming an issue as
  unrecoverable, when the service's timeline records exactly that event. That moved one count
  from 30 to 24. It was wrong in our favour, so it was found before the number was quoted.
- **2026-08-18**: an earlier claim that agents colliding on one object is not a real-world
  failure mode was based on twelve samples returning zero. Reading the remaining forty five
  returned five. The claim was withdrawn.

*Measured 18 August 2026. Written 19 August 2026. If you are reading this much later, re-run the
commands above rather than trusting the table.*
