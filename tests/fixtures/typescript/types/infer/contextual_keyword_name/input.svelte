<script lang="ts">
	// `infer T` declares a type parameter, whose name is a BindingIdentifier — so a
	// contextual keyword names one, exactly as it does in every other binding position
	type A<T> = T extends Array<infer string> ? string : never;

	type B<T> = T extends Array<infer any> ? any : never;

	type C<T> = T extends Array<infer undefined> ? undefined : never;

	// with a constraint, and with the name reused in both branches
	type D<T> = T extends Array<infer from extends object> ? from : never;
</script>
