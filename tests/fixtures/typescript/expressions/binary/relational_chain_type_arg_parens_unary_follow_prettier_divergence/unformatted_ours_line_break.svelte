<script lang="ts">
	// A `+` or `-` after a would-be type-argument close is a unary operator, so the
	// `<`…`>` is a comparison chain on any line — after a name, a member, a `new`
	// callee, and inside an argument list.
	a1 = f<T>
+c;
	a2 = f<T>
-c;
	a3 = a.b<T>
+c;
	a4 = new f<T>
-c;
	g(f<T>
+c);

	// Every other follower keeps the type arguments past a line break.
	b1 = f<T>
c;
	b2 = f<T>
(c);
	b3 = f<T>
`c`;
	b4 = f<T>
* c;
</script>
