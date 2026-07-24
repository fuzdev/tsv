<script lang="ts">
	// a same-line glued block directive before the value freezes the WHOLE union verbatim
	type A =   /* prettier-ignore */ a1&a2 | b1&b2;

	// extra internal whitespace is preserved verbatim under the whole-union freeze
	type B =    /* prettier-ignore */ a  |  b  |  c;
</script>
