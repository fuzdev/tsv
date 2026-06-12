<script>
	parent.body.nodes.filter(
		(n) =>
			n.type !== 'Comment' && n.type !== 'ConstTag' && (n.type !== 'Text' || n.data.trim() !== ''),
	).length > 1;
</script>
