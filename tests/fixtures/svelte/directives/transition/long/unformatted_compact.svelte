<!-- Single transition -->

<!-- 100 chars - stays inline (at boundary) -->
<div transition:fade={{duration:300,delay:100,easing:quintOut,x:0,yOffset:10000000}}></div>

<!-- 101 chars - wraps attributes -->
<div transition:fade={{duration:300,delay:100,easing:quintOut,x:0,yOffset:100000000}}></div>

<!-- With |local modifier -->

<!-- 100 chars - stays inline (at boundary) -->
<div transition:fade|local={{duration:300000,delay:10000,x:1000,yOffset:100000000000}}></div>

<!-- 101 chars - wraps attributes -->
<div transition:fade|local={{duration:3000000,delay:10000,x:1000,yOffset:100000000000}}></div>

<!-- in/out transitions -->

<!-- 100 chars - stays inline (at boundary) -->
<div in:fly={{x:100,y:0,duration:300,delay:10}} out:slide={{duration:200,delay:50}}></div>

<!-- 101 chars - wraps attributes, one per line -->
<div in:fly={{x:100,y:0,duration:300,delay:100}} out:slide={{duration:200,delay:50}}></div>

<!-- Object with many properties - properties wrap internally -->
<div transition:fade={{duration:300,delay:100,easing:quintOut,x:100,y:200,start:0.5,opacity:0}}></div>

<!-- tick function parameter -->
<div transition:fade={{duration:300,tick:(t,u)=>{opacity=t;transform=t;}}}></div>

<!-- css function parameter - arrow expression body -->
<div transition:fade={{duration:300,css:(t)=>'opacity: '+t+'; transform: scale('+t+');'}}></div>
