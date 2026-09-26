# Foundational Theorems of Modern Physics

::: theorem
**Theorem 1.1 (No-Cloning Theorem)**.
An arbitrary unknown quantum state cannot be cloned identically using any unitary transformation.
:::

::: proof
Assume a unitary operator $U$ such that $U(|\psi\rangle |e\rangle) = |\psi\rangle |\psi\rangle$ and $U(|\phi\rangle |e\rangle) = |\phi\rangle |\phi\rangle$. Taking the inner product yields $\langle \psi | \phi \rangle = (\langle \psi | \phi \rangle)^2$, which implies $\langle \psi | \phi \rangle \in \{0, 1\}$. Hence, cloning is impossible for non-orthogonal states. Q.E.D.
:::

::: definition
**Definition 1.2 (Entanglement Entropy)**.
For a bipartite pure state $|\psi_{AB}\rangle$, the entanglement entropy is defined as the von Neumann entropy of the reduced density matrix:
$$S(\rho_A) = -\text{Tr}(\rho_A \log_2 \rho_A)$$
:::

As demonstrated in seminal relativity theory [@einstein1905], spacetime curvature dictates the geodesic motion of freely falling particles.
