<script lang="ts">
// Comments between modifier keywords and names are preserved
class Base{a(){}}
class Sub extends Base{static/* c1 */a1=1;readonly/* c2 */b=2;accessor/* c3 */c=3;private/* c4 */d=4;async/* c5 */m1(){}
override/* c6 */a(){}}
</script>
