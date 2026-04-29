function startAuthPolling(sessionId) {
	const statusUrl = `/auth/status?sessionId=${encodeURIComponent(sessionId)}`;
	const pollInterval = 5000;
	let timerId;
	function updateStatusUI(status, message) {
		const statusElement = document.getElementById("auth-status");
		if (statusElement) {
			statusElement.textContent = message || status || "Unknown status";
			statusElement.className = `auth-status-${status}`;
		}
	}

	async function checkAuthStatus() {
		try {
			const response = await fetch(statusUrl);
			const data = await response.json();

			switch (data.status) {
				case "complete":
					clearInterval(timerId);
					updateStatusUI(
						"complete",
						"Authorization successful! Redirecting...",
					);

					setTimeout(() => {
						window.location.href = "/";
					}, 1500);
					break;

				case "pending":
					updateStatusUI("pending", "Waiting for authorization to complete...");
					break;

				case "failed":
					clearInterval(timerId);
					updateStatusUI(
						"failed",
						`Authorization failed: ${data.error || "Unknown error"}`,
					);
					break;

				case "expired":
					clearInterval(timerId);
					updateStatusUI(
						"expired",
						"Authorization session expired. Please try again.",
					);
					break;

				default:
					updateStatusUI(
						"unknown",
						`Unexpected status: ${data.status || "unknown"}`,
					);
			}
		} catch (error) {
			console.error("Error checking auth status:", error);
			updateStatusUI("error", "Error checking authorization status");
		}
	}

	timerId = setInterval(checkAuthStatus, pollInterval);

	checkAuthStatus();

	return function stopPolling() {
		clearInterval(timerId);
	};
}

async function handleLogin() {
	const loginEndpoint = "/login";
	try {
		const loginButton = document.getElementById("login-button");
		const loginStatus = document.getElementById("login-status");

		loginButton.disabled = true;
		loginStatus.textContent = "Connecting to Tidal...";

		const response = await fetch(loginEndpoint);
		const authData = await response.json();

		if (authData.error) {
			loginStatus.textContent = `Error: ${authData.error}`;
			loginButton.disabled = false;
			return;
		}

		const instructionsElement = document.getElementById("auth-instructions");
		if (instructionsElement) {
			instructionsElement.textContent = "";

			const codeDiv = document.createElement("div");
			codeDiv.className = "auth-code";
			codeDiv.textContent = authData.userCode;
			instructionsElement.appendChild(codeDiv);

			const p1 = document.createElement("p");
			p1.textContent = "Please visit ";
			const link = document.createElement("a");
			link.href = `https://${authData.verificationUriComplete}`;
			link.target = "_blank";
			link.textContent = authData.verificationUriComplete;
			p1.appendChild(link);
			instructionsElement.appendChild(p1);

			const p2 = document.createElement("p");
			p2.textContent = "Or go to ";
			const strongUri = document.createElement("strong");
			strongUri.textContent = authData.verificationUri;
			p2.appendChild(strongUri);
			p2.appendChild(document.createTextNode(" and enter code "));
			const strongCode = document.createElement("strong");
			strongCode.textContent = authData.userCode;
			p2.appendChild(strongCode);
			instructionsElement.appendChild(p2);

			const p3 = document.createElement("p");
			p3.textContent = `This code will expire in ${Math.floor(authData.expiresIn / 60)} minutes`;
			instructionsElement.appendChild(p3);

			const statusDiv = document.createElement("div");
			statusDiv.id = "auth-status";
			statusDiv.className = "auth-status-pending";
			statusDiv.textContent = "Waiting for authorization...";
			instructionsElement.appendChild(statusDiv);

			instructionsElement.style.display = "block";
		}

		loginButton.style.display = "none";
		loginStatus.style.display = "none";

		startAuthPolling(authData.sessionId);
	} catch (error) {
		console.error("Login error:", error);
		const loginStatus = document.getElementById("login-status");
		const loginButton = document.getElementById("login-button");
		if (loginStatus) loginStatus.textContent = "Error connecting to Tidal. Please try again.";
		if (loginButton) loginButton.disabled = false;
	}
}

document.addEventListener("DOMContentLoaded", () => {
	const loginButton = document.getElementById("login-button");
	if (loginButton) {
		loginButton.addEventListener("click", handleLogin);
	}

	const urlParams = new URLSearchParams(window.location.search);
	const sessionId = urlParams.get("session");
	if (sessionId) {
		const loginButton = document.getElementById("login-button");
		const instructionsElement = document.getElementById("auth-instructions");

		if (loginButton) loginButton.style.display = "none";
		if (instructionsElement) {
			instructionsElement.textContent = "Checking authorization status...";
			instructionsElement.style.display = "block";
		}
		startAuthPolling(sessionId);
	}
});
