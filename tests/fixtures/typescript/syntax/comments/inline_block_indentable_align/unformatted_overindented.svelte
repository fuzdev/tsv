<script lang="ts">
	// An INDENTABLE multi-line block comment does not reprint as its own source span:
	// its continuation lines are re-indented to the column it is emitted at. That holds
	// in the inline list positions too, where the comment shares a line with the item it
	// leads or trails.

	// a sole type argument, leading and trailing
	const a: A</** c
		 */ T> = b;
	const c: A<T /** c
		 */> = d;

	// a type-parameter list
	function e<
		T /** c
			 */
	>(f: T) {}

	// an import specifier
	import {
		g /** c
			 */,
		h
	} from './a';

	// a tuple member
	const i: [
		A /** c
			 */,
		B
	] = j;

	// a function-type parameter
	type K = (
		l: A /** c
			 */,
		m: B
	) => C;

	// an import attribute's key, whose own gap hangs nothing
	import n from './b' with {
		type /** c
			 */: 'json'
	};

	// Contrast: a PRESERVED (non-indentable) block prints verbatim, so its own
	// authored columns are the output at every one of these positions.
	const o: A<T /* c1
   c2 */> = p;
	const q: [
		A /* c1
   c2 */,
		B
	] = r;
</script>
