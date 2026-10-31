import { createRouter, createWebHashHistory } from "vue-router";
import ConfigureView from "./views/ConfigureView.vue";
import HomeView from "./views/HomeView.vue";
import PlanView from "./views/PlanView.vue";
import ResultView from "./views/ResultView.vue";
import ScanProgressView from "./views/ScanProgressView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "home", component: HomeView },
    { path: "/configure", name: "configure", component: ConfigureView },
    { path: "/scan", name: "scan-progress", component: ScanProgressView },
    { path: "/plan", name: "plan", component: PlanView },
    { path: "/result", name: "result", component: ResultView },
  ],
});
