<script>
	// A postfix update ends an `UpdateExpression`, which no call, member or template
	// continues, so under a prefix operator too a line opening with `(`, `[` or a
	// template is a new statement.
	a1 = !b++;
	z;
	a2 = -b--;
	z;
	a3 = typeof b.c++;
	z;
	a4 = void b[0]--;
	[0];
	a5 = !!b++;
	`t`;
	a6 = !b++;
	(z) => z;
</script>
