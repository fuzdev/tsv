<script lang="ts">
	// case C: first intersection member with leading line comment
	type C = ((// leading
	a)) & b;

	// case M3: nested union inside intersection - the shell wraps the whole value, so it
	// is the one that strips (a shell around the paren-union MEMBER is retained and keeps
	// the comment inside, in intersection_retained_paren_first_member_leading_line_comment)
	type M3 = ((// leading
	(a | b) & c));
</script>
