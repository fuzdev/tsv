<script lang="ts">
// Four comment positions inside the index-signature brackets, interface context:
interface A {
[/* a */k:string]:A;
}
interface B {
[k/* b */:string]:A;
}
interface C {
[k:/* c */string]:A;
}
interface D {
[k:string/* d */]:A;
}
// Same four positions, type-literal context:
type E = {
[/* e */k:string]:A;
};
type F = {
[k/* f */:string]:A;
};
type G = {
[k:/* g */string]:A;
};
type H = {
[k:string/* h */]:A;
};
</script>
