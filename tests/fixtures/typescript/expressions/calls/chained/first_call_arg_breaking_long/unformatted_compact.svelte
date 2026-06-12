<script>
	// 100 visual chars (98 content + 1 tab): fits on one line
	expect(obj_name_.method_n(CONST___.OUTSIDE, 'content')).rejects.toThrow('aaaaaaaaaaaaaaaaaaaaaa');

	// 101 visual chars (99 content + 1 tab): break the LAST call's args, not the FIRST
	expect(obj_name_.method_n(CONST____.OUTSIDE, 'content')).rejects.toThrow('aaaaaaaaaaaaaaaaaaaaaa');
</script>
