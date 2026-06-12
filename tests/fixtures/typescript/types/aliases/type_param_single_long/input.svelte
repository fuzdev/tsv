<script lang="ts">
	// single-type `extends` constraint overflows: break after `extends`, indent the type
	type Constraint<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAA extends
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
	> = X;

	// single-type `=` default overflows: break after `=`, indent the type
	type Default<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA =
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
	> = X;
</script>
