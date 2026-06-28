<script lang="ts">
	// union `extends` constraint breaks after `extends`; the union stays inline
	type Constraint<AAAAAAAAAAAAAAAAAAAAAAAAA extends Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Cccccccccccccccccccccccccccccccccc> = X;

	// union default breaks after `=` the same way
	type Default<AAAAAAAAAAAAAAAAAAAAAAAAAAAAA = Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Cccccccccccccccccccccccccccccccccc> = X;
</script>
