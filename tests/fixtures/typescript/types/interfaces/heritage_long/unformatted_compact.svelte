<script lang="ts">
// Medium length - stays on one line (85 chars)
interface LongHeritage<ES extends TypeOne|TypeTwo>extends Base1<ES>,Base2<ES>{prop:string;}

// Very long - type params break, heritage on separate line
interface VeryLong<T extends VeryLongTypeName|AnotherLongTypeName,U extends ExtraLongConstraintType|MoreTypes>extends Base1<T>,Base2<T>,Base3<U>{prop:string;}

// Short - stays on one line (72 chars, contrast case)
interface Short<T extends string|number>extends Base<T>,Other<T>{prop:T;}
</script>
