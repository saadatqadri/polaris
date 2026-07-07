# The Factory Is the Wrong Metaphor

The phrase is everywhere now. *Software factory.* Plants humming, lines moving, agents producing artifacts around the clock. It's meant to sound like progress — like we finally industrialized the messy craft of building software. I have a growing repulsion to it, and it took me a while to articulate why.

image: 

A factory is a sterile environment running a linear process. That isn't an insult; it's the definition. The entire genius of a factory is that it takes a process you already understand completely and eks out margin through incremental improvement to it. Toyota didn't invent the car on the line. The asssemby line came *after* the car was known. A factory's whole reason for existing is that the hard question — *what are we making, and why does it work* — has already been answered. All that's left is to make it cheaper, faster, more uniform.

That is the opposite of how good software gets *discovered.*

## The case in point

Claude Code is the obvious example. I’ve been watching Claude Code emerge from the very start, and the evolution of the product has felt “organic”. I feel seen and heard by the team building Claude Code. 

Nobody *could* have. It was set loose in the wild as a fairly amorphous block, and the shape came from watching what people actually did with it - what they reached for, what they abused, where they found leverage nobody anticipated. The value emerged from contact with real usage, not before it. The roadmap was downstream of the release, not upstream.

If you'd run that as a factory process, you'd have specced the thing to death, optimized a production line for a product that didn't yet know what it was, and shipped a beautifully manufactured answer to the wrong question.

This is essentially what YC has preached for years, and their most successful disciples have discovered as the truth: at the early stages of product development, there is barely a plan. The early stage product exists only as a means of gathering feedback from real users. 

This is the part the factory metaphor quietly smuggles in: the assumption that you already know what you're building. For frontier software, you almost never do. You're not optimizing a known process. You're trying to discover what the process even is.

## The better metaphor, and its trap

So the metaphor I keep landing on is an organism. Something that grows, gets reshaped, responds to stimuli and feedback. Something with impulses and a nervous system — picking up the thread from the antibodies piece. Software is malleable, and the best software metabolizes feedback rather than rolling off a line.

But I want to be careful here, because "organism" has its own failure mode, and it's a seductive one. It becomes the excuse for *no rigor at all.* "We're not a factory, man, we're just letting it grow." That's not biology. That's neglect with good branding.

Because here's the thing about actual organisms: they are not undisciplined. They are *ruthlessly selective.* An immune system isn't gentle. Evolution isn't permissive. A living system runs a brutally tight loop of variation and culling — generate widely, kill almost everything, keep the rare thing that survives contact with the environment. The discipline isn't in the planning. The discipline is in the selection.

And that's the actual claim, the one underneath the metaphor:

**A factory optimizes a process you already know. An organism is how you discover what the process is. The first runs on *specification.* The second runs on *selection.*** Most of the "software factory" discourse is really just specification cosplay — the comforting fantasy that we can spec our way to a product whose shape can only be found by releasing it.

The work isn't growth versus process. Both modes have process. The work is knowing which mode you're in — and if you're in discovery, your discipline has to live in how aggressively you select, not in how completely you planned.

## Why now

Here's the part that makes this urgent rather than philosophical, and it's the part most of the discourse skips: the organism was *always* the better discovery process. It simply wasn't affordable.

Selection mode has a brutal prerequisite. To run it, you have to generate     //widely and cull most of what you generate. Evolution works because variation is cheap and death is free. For nearly all of software history, neither was true. Producing a single variant cost weeks of expensive engineer time. When each    variation is that costly, you *cannot* afford to make ten and keep one — you'd go broke. So the rational move was to front-load specification: minimize the number of expensive variants you commit to, and try to be right on paper before you spent the money. The factory metaphor wasn't stupid. It was a correct adaptation to the cost of variation.

Coding agents broke that constraint. Generating a variant is now cheap and fast — minutes, not weeks. The thing that made selection mode economically ruinous just collapsed. For the first time, you can actually afford to build the way an organism builds: vary widely, expose it to the environment, kill almost everything, keep what survives. The metaphor isn't newly *nicer*. It's newly *affordable*.

And velocity is only half of it. The other half is attention. When agents absorb 
execution, the scarcest human resource stops being the ability to write the code and becomes the judgment to read what comes back — which variant survived contact, what the behavior is telling you, what to kill. We used to spend our attention on production. Now it's freed, and it has to be spent on selection. That isn't a side effect of the shift. That *is* the shift.

Which raises the stakes rather than lowering them. Cheap variation also means cheap slop. The old constraint — *can we even afford to build this?* — used to do your culling for you; scarcity was a filter that ran for free. Remove it and nothing gets killed automatically anymore. The bottleneck moved from generation to selection. If your selection loop can't keep pace with what your agents can now produce, you don't get an organism. You get a tumor.

## The steelman: the factory is an organ

I've set that up too cleanly, and the strongest version of the other side deserves a straight answer. The serious factory pitch isn't the sterile line I described. Factory.ai — the sharpest instance — doesn't sell a dumb conveyor belt; it sells a self-improving system that ingests continuous signals and ships production software, with triage, code generation, validation, release, and monitoring each tuning itself. They've already absorbed the feedback loop. So "a factory can't respond to stimuli" is a strawman, and I don't want to win against a strawman.

The honest answer came from an unlikely place: your own body runs a factory. The production of white blood cells is, mechanically, exactly that. It happens on signal. It runs without drama. It's standardized, high-throughput, ruthlessly repeatable. An organism doesn't reject the factory — it *contains* one. Bone marrow is a production line.

That's the resolution, and it's why the factory is the wrong metaphor for the *whole* rather than a wrong idea outright. The factory isn't the opposite of the organism. It's an *organ.* Specification mode isn't a mistake — it's the correct tool for the parts of the work that are known and repeatable, and agents make those production lines cheap to run around the clock. Regression suites, dependency bumps, code review, ticket-to-code on a well-specified ticket, docs that regenerate on change: that's marrow. Run it 24/7. No notes.

But watch what the marrow does *not* do. It doesn't decide which pathogen is the threat. It doesn't choose what to mass-produce, or when to ramp. That decision lives upstream, in the signaling that reads the environment and commits the body to a response. A self-tuning production line is still a production line — it gets better at making *what it already knows to make.* No amount of throughput optimization on the code-gen stage tells you whether the thing should exist. Production is not discovery.

So the two aren't enemies; they're different organs, and the interesting companies are each building one. Factory.ai is building the marrow — the production line of the known, tuning itself for throughput. Trajectory.ai is building the other half: capture the signals real usage already generates (corrections, edits, re-prompts, traces), learn from what survives contact, gate the change on human judgment, deploy. That's not a production line. That's the selection system — the immune loop deciding what's worth keeping. One optimizes execution of the known. The other discovers what becomes known.

A healthy system needs both, wired in one specific order: **selection governs, the factory serves.** The immune system decides; the marrow produces. Reverse it — let the production line's efficiency stand in for the judgment about what to produce — and you don't get a leaner organism. You get a tumor, which is precisely a production line that escaped the selection system. It is *extremely* good at throughput. That's the entire problem with it.

---

## The artifact: Specification mode vs. Selection mode

A mature system runs both modes — the point isn't to pick one, it's to route each piece of work to the mode that fits and keep them in the right order. The known and repeatable goes to the factory; the unknown goes to selection; and selection sits upstream, deciding what the factory is allowed to mass-produce. Most teams get this wrong in two ways: they default to specification mode everywhere because it *feels* like rigor, or they run a factory with no selection system above it and mistake throughput for progress. The rigor doesn't disappear in selection mode — it relocates from the plan to the loop.

| | **Specification mode** (factory) | **Selection mode** (organism) |
|---|---|---|
| **You know…** | what to build, and why it works | the rough direction; the shape is unknown |
| **The scarce skill** | execution against a known target | killing things fast and reading what survives |
| **Where rigor lives** | in the plan, up front | in the selection loop, continuously |
| **Primary risk** | building the wrong thing perfectly | drift — growth without culling |
| **What "done" means** | matches the spec | the environment stopped rejecting it |
| **Wrong move** | re-spec when reality disagrees | "let it grow" with nothing being killed |

**How to run selection mode without it becoming neglect:**

1. **Release before you're sure.** The amorphous block in the wild teaches you more in a week than the spec did in a month. Contact with reality is the input, not the final exam.
2. **Generate wide, cull hard.** The output isn't the thing you build — it's the thing that *survives.* If you're not killing most of what you make, you're not selecting, you're just accumulating.
3. **Make feedback a sense organ, not a survey.** Instrument for what people actually reach for, not what they say. Selection runs on behavior, not opinion.
4. **Name the selection pressure explicitly.** An organism without an environment doesn't evolve, it just sprawls. What, precisely, is allowed to kill a feature? If nothing is, you're back to neglect.
5. **Graduate work into the factory, and keep it subordinate.** Once a thing's shape is known and stops surprising you, hand it to the line and let it run 24/7 — that's what the line is for. But the line answers to selection, never the other way around. The mistake isn't building factories. It's building them in discovery, or letting a factory's efficiency decide what gets made.

The factory people aren't wrong to build the line. Agents make that line cheaper and faster than it has ever been, and that's real leverage. They're wrong about what it answers to. A production line is an organ, not an organism — and an organ that decides for itself what to produce has a name in biology, and it isn't "efficient." It's malignant.