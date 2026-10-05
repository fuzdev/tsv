<script lang="ts">
	// A relational chain whose `<` operand keeps a paren shell of its own takes the outer
	// pair too. The printed region then opens on that `(`, and a `(`-headed region is a
	// type-argument list wherever a line terminator, a `(` or a template follows the
	// matching `>` — so the bare spelling is an output that re-lexes as an instantiation.
	// The shell survives on every operand that binds looser than `<`, and on `await` and
	// `yield`, whose pair is a clarity one.

	// Assignment and compound assignment.
	const k1 = x < (a = b) > c;
	const k2 = x < (a += b) > c;

	// A conditional and the three logical operators.
	const k3 = x < (a ? b : q) > c;
	const k4 = x < (a || b) > c;
	const k5 = x < (a && b) > c;
	const k6 = x < (a ?? b) > c;

	// Equality and `^`, which bind looser than a relational operator.
	const k7 = x < (a == b) > c;
	const k8 = x < (a ^ b) > c;

	// The relational keywords.
	const k9 = x < (a in b) > c;
	const k10 = x < (a instanceof b) > c;

	// `await`, whose shell is a clarity pair both formatters synthesize from the bare
	// authoring.
	const k11 = x < (await a) > c;

	// `as` and `satisfies`, which end on a type rather than on an expression.
	const k12 = x < (a as T) > c;
	const k13 = x < (a satisfies T) > c;

	// The follower is no protection: past a `(` or a template the region commits with no
	// line break anywhere.
	const k14 = x < (a = b) > (t, u);
	const k15 = x < (a = b) > `t`;

	// A kept shell whose content SPELLS a type asks no new question — the region-keyed
	// rule already reads a sequence as the argument separator and `() =>` as a function
	// type, so the pair stands there for the reason it stands everywhere else.
	const k16 = x < (a, b) > c;
	const k17 = x < (() => a) > c;

	// A JSDoc cast is a shell of a third kind: its parens are semantically required, so its
	// own doc prints them whatever the position asks, and the head scan steps over the
	// comment to reach the `(` exactly as it would without one.
	const k18 = x < /** @type {T} */ (a = b) > c;

	// The region's head is the FIRST PRINTED BYTE, not the operand's own, so a `(` the
	// leftmost printed spine inherits opens it too. A computed member prints `[` between
	// its object and the rest of the region, and `[` is the one postfix a parenthesized
	// type takes, so the pair reaches through one — and through a run of them.
	const k20 = x < (a = b)[0] > c;
	const k21 = x < (a = b)[0][1] > c;
	const k22 = x < (a as T)[0] > c;
	const k23 = x < (await a)[0] > c;
	const k24 = x < (a, b)[0] > c;
	const k25 = x < /** @type {T} */ (a = b)[0] > c;

	// Every other spine hop puts a token after the shell's `)` that continues no type — a
	// `.` (a qualified name needs an identifier head), a call's `(`, a `!`, an operator,
	// and an optional member's `?` — so the chain stays bare through all of them.
	const k26 = x < (a = b).c > c;
	const k27 = x < (a = b)! > c;
	const k28 = x < (a = b)() > c;
	const k29 = x < (a = b) + 1 > c;
	const k30 = x < (a = b)?.[0] > c;

	// An ARGUMENT LIST is the other token that may stand directly behind the shell: tsv's
	// own parse keeps a region claimed where a `<` follows a shell that heads it. Where the
	// shell holds what reads as a type name — a `typeof` query, a `!`, a union — the pair
	// reaches through an instantiation, a generic call's callee and a generic tagged
	// template's tag, and through a computed member behind one.
	const k36 = x < (typeof a)<T>(d) > c;
	const k37 = x < (!a)<T>`t` > c;
	const k38 = x < (a | b)<T>(d)[0] > c;
	// whatever stands above the list on the spine: a member, a second call, an
	// optional call, an arithmetic tail
	const k42 = x < (typeof a)<T>(d).m > c;
	const k43 = x < (typeof a)<T>(d)(g) > c;
	const k44 = x < a | b<T>?.(d) > c;
	const k45 = x < (!a[0])<T>(d) + 1 > c;

	// A shell that parse refuses by its content — a logical or arithmetic operator, `as`,
	// a sign on a name, a union over a call — is a comparison operand whatever follows it,
	// and the chain stays bare.
	const k39 = x < (a ?? b)<T>(d) > c;
	const k40 = x < (a as T)<U>`t` > c;
	const k41 = x < (-a)<T>(d) > c;
	const k46 = x < (a | b())<T>(d) > c;

	// A REGEX literal inside the shell is the one thing a delimiter scan can misread: an
	// unescaped `)` in its pattern closes nothing, so the group's own `)` — and the byte
	// past it, which is what a `(…) =>` reading keys on — are reachable only by a walk
	// that steps over the literal whole. A character class hides one the same way.
	const k31 = x < (a, /\)=>/) > c;
	const k32 = x < (a, /\)=>b/) > c;
	const k33 = x < (a, /[)]=>b/) > c;
	const k34 = x < (/\)=>b/, a) > c;
	const k35 = x < (a, (b, /\)=>c/)) > c;

	// `yield` asks the same question inside its generator.
	function* fn() {
		const k19 = x < (yield a) > c;
	}
</script>
