<script lang="ts">
class A {
[/* a */k:string]:T;
}
class B {
[k/* b */:string]:T;
}
class C {
[k:/* c */string]:T;
}
class D {
[k:string/* d */]:T;
}
</script>
