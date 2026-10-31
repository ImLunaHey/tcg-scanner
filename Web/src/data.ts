import { useQuery } from "@tanstack/react-query";
import { useAuth } from "./auth";
import type { Device, JobPage, Profile, ServiceStatus } from "./models";

export function useWorkspace() {
  const { api } = useAuth();
  const status = useQuery({
    queryKey: ["status"],
    queryFn: ({ signal }) => api.get<ServiceStatus>("status", signal),
    refetchInterval: 5000,
  });
  const devices = useQuery({
    queryKey: ["devices"],
    queryFn: ({ signal }) => api.get<{ devices: Device[] }>("devices", signal),
    refetchInterval: 10_000,
  });
  const profiles = useQuery({
    queryKey: ["profiles"],
    queryFn: ({ signal }) =>
      api.get<{ profiles: Profile[] }>("profiles", signal),
  });
  const jobs = useQuery({
    queryKey: ["jobs", "recent"],
    queryFn: ({ signal }) => api.get<JobPage>("jobs?limit=10", signal),
    refetchInterval: 5000,
  });
  return { status, devices, profiles, jobs };
}
