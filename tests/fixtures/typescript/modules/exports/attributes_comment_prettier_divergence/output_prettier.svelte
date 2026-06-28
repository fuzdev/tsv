<script>
	// re-export named: source → with (block comment)
	export { a } from 'a' /* c1 */ with { type: 'json' };

	// re-export named: with → brace
	export { b } from 'b' /* c2 */ with { type: 'json' };

	// re-export named: after attributes
	export { c } from 'c' with { type: 'json' }; /* c3 */

	// re-export all: with → brace
	export * from 'd' /* c4 */ with { type: 'json' };

	// re-export all: after attributes
	export * from 'e' with { type: 'json' }; /* c5 */
</script>
