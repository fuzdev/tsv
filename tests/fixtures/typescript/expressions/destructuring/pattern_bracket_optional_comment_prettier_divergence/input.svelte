<script lang="ts">
	// A block comment between a destructuring pattern's closing bracket and its
	// optional `?` marker stays on the side the author wrote it.
	type F1 = ({ a } /* c1 */?: T) => void;
	type F2 = ([b] /* c2 */?: U) => void;

	// An empty pattern takes the same rule.
	type F3 = ({} /* c3 */?: V) => void;
	type F4 = ([] /* c4 */?: W) => void;

	// The two gaps the marker separates are independent, so a comment on each
	// side keeps its own side.
	type F5 = ({ c } /* c5 */? /* c6 */ : A) => void;

	// The marker gap alone, with no annotation behind it.
	type F6 = ({ d } /* c7 */?) => void;
	type F7 = ([e] /* c8 */?) => void;
	type F8 = ({} /* c9 */?) => void;
</script>
