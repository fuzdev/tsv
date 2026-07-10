<script lang="ts">
	// trailing line comment on an object member glued to a following member
	type D = A &
		B & { c: C } & D & // c
		E;

	// trailing line comment on a member glued to a following object
	type G = A &
		B & { c: C }; // c
</script>
