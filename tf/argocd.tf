terraform {
  required_providers {
#     helm = {
#       source  = "hashicorp/helm"
#       version = "2.12.1"
#     }
#
#     kubernetes = {
#       source  = "hashicorp/kubernetes"
#       version = "2.25.2"
#     }
    kubectl = {
      source  = "gavinbunney/kubectl"
      version = ">= 1.10.0"
    }
  }
}

provider "helm" {
  kubernetes {
    config_path = "/home/runner/kubeconfig"
  }
}

provider "kubernetes" {
  config_path = "/home/runner/kubeconfig"
}

resource "kubernetes_namespace" "infrastructure" {
  metadata {
    name = "infrastructure"
  }
}

resource "helm_release" "argocd" {
  name       = "argocd"
  chart      = "argo-cd"
  repository = "https://argoproj.github.io/argo-helm"
  version    = "6.0.12"
  namespace  = kubernetes_namespace.infrastructure.metadata[0].name

  values = ["${file("argocd-values.yml")}"]
}

resource "helm_release" "nginx_ingress" {
  name       = "ingress-nginx"
  chart      = "ingress-nginx"
  repository = "https://kubernetes.github.io/ingress-nginx"
  version    = "4.9.1"
  namespace  = kubernetes_namespace.infrastructure.metadata[0].name

  set {
    name  = "controller.ingressClassResource.name"
    value = "nginx"
  }
}
