<script lang="ts">
	// a declaration as a clause body: the pre-`;` comment run keeps its lines, at the `if`'s level
	if (a) type X = Y;
	// c1
	// c2
	else fn1();
	if (b) declare function g(): void;
	// c3
	else fn2();
	if (c) declare module 'm';
	// c4
	else fn3();
	// a glued run in that same gap breaks between its members, the answer the gap gives
	// once the `;` sits ahead of the run
	if (d) type Z = W;
	/* c5 */
	/* c6 */
	else fn4();
	if (e) declare function h(): void;
	/* c7 */
	// c8
	else fn5();
	if (f) declare module 'n';
	/* c9 */
	/* c10 */
	/* c11 */
	else fn6();
</script>
