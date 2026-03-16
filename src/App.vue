<script setup>
import { RouterView } from "vue-router"
import Sidebar from "./components/Sidebar.vue"
</script>

<template>

<div class="layout">

  <header class="topbar">
    <div class="logo">DB Dumper</div>
  </header>

  <div class="main">

    <aside class="sidebar">
      <Sidebar />
    </aside>

    <section class="content">
      <RouterView />
    </section>

  </div>

</div>

</template>

<style>

html,body,#app{
  height:100%;
  margin:0;
  font-family: system-ui, -apple-system, sans-serif;
}

.layout{
  display:flex;
  flex-direction:column;
  height:100%;
}

.topbar{
  height:40px;
  background:#2c2c2c;
  color:white;
  display:flex;
  align-items:center;
  padding:0 12px;
  font-size:14px;
}

.main{
  flex:1;
  display:flex;
  overflow:hidden;
}

.sidebar{
  width:260px;
  background:#1e1e1e;
  color:#ddd;
  overflow:auto;
  border-right:1px solid #333;
}

.content{
  flex:1;
  background:#fafafa;
  padding:20px;
  overflow:auto;
}

</style>