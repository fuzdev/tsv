<script lang="ts">
	// Comment between key and in keyword
	type A = {[a /* commentA */ in A]: string};

	// Comment between source type and ]
	type B = {[b in B /* commentB */]: string};

	// Comment before key inside brackets
	type C = {[/* commentC */ c in C]: string};

	// Comment after in keyword
	type D = {[d in /* commentD */ D]: string};

	// Comment between ] and :
	type E = {[e in E /* commentE */]: string};

	// Comment between source and as keyword
	type F = {[f in F /* commentF */ as G]: string};

	// Comment after as keyword
	type G = {[g in G as /* commentG */ H]: string};

	// Comment between as target and ]
	type H = {[h in H as I /* commentH */]: string};

	// Multiple comment positions
	type I = {[/* c1 */ i /* c2 */ in /* c3 */ I /* c4 */]: string};
</script>
