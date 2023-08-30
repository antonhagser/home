# Integrated Smart Home Ecosystem

Services for advanced home automation and monitoring.

## Hardware and software

The services, unless specified otherwise, are docker containers running in a kubernetes cluster managed by [K3s](https://k3s.io/) powered by a Intel NUC.

The kubernetes cluster has a ingress controller powered by [Istio](https://istio.io/) and manages certificates with [cert-manager](https://cert-manager.io/).

The services are made into docker images and pushed to a private docker registry powered by [Harbor](https://goharbor.io/) running in the cluster.

Some services utilize [argocd](https://argoproj.github.io/argo-cd/) for continuous deployment and take advantage of kubernetes rollouts for zero downtime deployments. (although a bit overkill for a home automation setup)

## Services and devices

The following services are currently in use:

### Home utilities

Services used for monitoring and controlling utilities in the house.

#### [Electricity](./electricity/README.md) - Electricity consumption monitoring

The `electricity` service monitors the electricity consumption of the house, it does this by aggregating the data from the `p1meter` service and from a modbus connection available on the solaredge smart solar inverter.

[Grafana](https://grafana.com) is deployed in the cluster and is used to visualize the data:

![Grafana electricity dashboard](<./images/grafana-electricity.png> "Grafana electricity dashboard")

#### [P1 Meter](./p1meter/README.md) - P1 energy meter reader

The `p1meter` is powered by an ESP-12F and runs firmware which connects to the LAN and transmits the latest DSMR 5.0.2 data from the P1 han-port of the smart meter to a TCP socket running on the `electricity` ingest service.
