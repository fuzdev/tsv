<script lang="ts">
	// A brand check with no class in scope at all. The confining rule is
	// `AllPrivateIdentifiersValid` — a whole-Script EARLY error rather than a grammar one, so
	// tsv defers it like every other early error it cannot answer from local context
	const a = #x in y;

	// The same production one nesting level in, still with no class in scope
	function fn(y: object) {
		return #x in y;
	}

	// INSIDE a class body, but the name is not bound by it — the same deferral, since the
	// ecma262 rule is binding rather than containment. acorn says so in as many words
	// ('Private field '#nope' must be declared in an enclosing class'); tsc's parser
	// accepts, leaving only a semantic diagnostic on the property
	class C {
		m(y: object) {
			return #nope in y;
		}
	}
</script>
