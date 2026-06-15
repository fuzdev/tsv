<script>
	// re-export named: source → with (block, dual-stable)
	export {a} from 'a' /* c1 */ with {type: 'json'};

	// re-export named: with → brace (tsv keeps; prettier relocates before `with`)
	export {b} from 'b' /* c2 */ with {type: 'json'};

	// re-export named: after attributes (tsv keeps after `}`; prettier moves inside braces)
	export {c} from 'c' with {type: 'json' /* c3 */};

	// re-export all: with → brace
	export * from 'd' /* c4 */ with {type: 'json'};

	// re-export all: after attributes
	export * from 'e' with {type: 'json' /* c5 */};
</script>
