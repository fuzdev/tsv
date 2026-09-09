<script lang="ts">
	// A blank line a statement's own terminator sits below belongs to the statement,
	// so it survives the `;` moving back up to the content it terminates.
	a1()

	;
	b1();

	// Every kind measured from its content end rather than its full end.
	import c1 from './a'

	;
	import type { C2 } from './b'

	;
	export const d1 = 1

	;
	export * from './c'

	;
	var e1 = 1,
		e2 = 2

	;
	declare const e3: number

	;
	debugger

	;
	b2();

	// A header kind is measured through its body, so the rule reaches the body's own
	// terminator.
	l1: a2()

	;
	if (cond) a3()

	;
	if (cond) a4();
	else a5()

	;
	for (;;) a6()

	;
	for (const f1 in obj) a7()

	;
	for (const f2 of arr) a8()

	;
	while (cond) a9()

	;
	l2: while (cond) a10()

	;
	do a11();
	while (cond)

	;
	b3();

	function fn1() {
		'use strict'

		;
		return a12

		;
		b4();
	}

	function fn2() {
		throw a13

		;
		b5();
	}

	while (cond) {
		break

		;
		b6();
	}

	l3: while (cond) {
		continue l3

		;
		b7();
	}

	// A kind the table does not list is measured from its full end, so a blank above
	// its terminator stays inside the statement and never reaches the gap.
	type A1 = B1

	;
	b8();

	// A header whose body IS the terminator is measured through that empty statement,
	// which the table does not list either.
	for (;;)

	;
	while (cond)

	;
	l4:

	;
	if (cond)

	;
	b9();

	// A terminator on the next line with no blank above it gains none.
	a14()
	;
	b10();

	// A comment the author trailed on the content's own line is skipped the way
	// prettier's own scan skips it, so the blank below it still reaches the gap.
	a18() // c1

	;
	a19() /* c2 */

	;
	b14();

	// A comment sitting BELOW the blank owns that blank itself, and a comment between
	// the content and its terminator leaves the question to the terminator's own end.
	a20()

	// c3
	;
	a21() /* c4 */ ;

	b15();

	// Block body, switch consequent and namespace body take the same rule.
	{
		a15()

		;
		b11();
	}

	switch (cond) {
		case 1:
			a16()

			;
			b12();
	}

	namespace N1 {
		a17()

		;
		b13();
	}

	// A case LABEL gap is measured from the SwitchCase, a kind the table does not list, so a
	// blank above a detached terminator stays inside the consequent and never reaches it.
	switch (cond) {
		case 1:
			a23()

			;
		case 2:
			b17();
	}

	// The null control for that gap: a blank the author wrote BELOW the terminator is the
	// gap's own and is kept.
	switch (cond) {
		case 3:
			a24();

		case 4:
			b18();
	}

	// A frozen statement's terminator belongs to the printer too, so the blank below its
	// content reaches the gap by this same rule.
	// prettier-ignore
	a25 (   )

	;
	b19();
</script>
