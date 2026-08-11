<script>
	// block comment between the attribute `}` and `;` trails after the `;`
	import a from './a' with {type:'json'}/* c */;

	// line comment between the attribute `}` and `;` trails after the `;`
	import b from './b' with {type:'json'}// c
	;

	// a whole run in that gap trails as one, in source order
	import c from './c' with {type:'json'}/* 1 */// 2
	;

	// a line comment INSIDE the braces keeps the clause expanded
	import d from './d' with {type:'json'// c
	};

	// both sides at once: the interior comment expands the clause, the gap comment
	// still trails the `;`
	import e from './e' with {type:'json'// interior
	}// gap
	;

	// the empty clause has the same gap
	import f from './f' with {}// c
</script>
