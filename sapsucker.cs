// Assembly-CSharp, Version=0.0.0.0, Culture=neutral, PublicKeyToken=null
// GiantKiwiAI
using System;
using System.Collections.Generic;
using System.Linq;
using GameNetcodeStuff;
using Unity.Netcode;
using UnityEngine;
using UnityEngine.AI;
using UnityEngine.Animations.Rigging;

public class GiantKiwiAI : EnemyAI, IVisibleThreat
{
	public bool inKillAnimation;

	[Space(7f)]
	public float idlePatrolSpeed = 8f;

	public GameObject eggPrefab;

	public List<KiwiBabyItem> eggs = new List<KiwiBabyItem>();

	private bool syncedEggsPosition;

	private bool hasSpawnedEggs;

	private int idleBehaviour;

	private float behaviourTimer;

	private float idleTimer;

	private int previousBehaviour;

	public Transform[] nestEggSpawnPositions;

	private System.Random idleRandom;

	private int previousIdleBehaviour;

	private Collider[] treeColliders;

	public Transform peckingTree;

	private bool isPeckingTree;

	public float peckTreeDistance = 4f;

	private float miscTimer;

	private Vector3 agentLocalVelocity;

	private float velX;

	private float velZ;

	public Transform animationContainer;

	private Vector3 previousPosition;

	[Header("Animation values")]
	private bool inMovement;

	[Header("Audios")]
	public AudioClip[] footstepSFX;

	public AudioClip[] footstepBassSFX;

	public AudioClip peckTreeSFX;

	public AudioSource peckAudio;

	public AudioSource longDistanceAudio;

	public DampedTransform dampedHeadTransform;

	public ParticleSystem woodChipParticle;

	[Header("Sight")]
	public Transform lookTarget;

	public Transform headLookTarget;

	public Transform eyesLookTarget;

	public MultiAimConstraint headLookRig;

	public MultiAimConstraint eyesLookRigA;

	public MultiAimConstraint eyesLookRigB;

	private float pingAttentionTimer;

	private int focusLevel;

	private Vector3 pingAttentionPosition;

	private float timeSincePingingAttention;

	private float timeAtLastHeardNoise;

	public Collider ownCollider;

	private int visibleThreatsMask = 524296;

	private int lookMask = 1073744129;

	private IVisibleThreat seenThreat;

	private IVisibleThreat watchingThreat;

	public Transform watchingThreatTransform;

	public Transform turnCompass;

	public Transform leftEyeMesh;

	public Transform rightEyeMesh;

	public Transform neck2;

	private float eyeTwitchInterval;

	private float turnHeadInterval;

	private float turnHeadOffset;

	public float eyeTwitchAmount;

	private Vector3 baseEyeRotation;

	private Vector3 baseEyeRotationLeft;

	[Header("Nest behaviour")]
	public float protectNestRadius = 7f;

	private List<KiwiBabyItem> LostEggs = new List<KiwiBabyItem>(3);

	private List<KiwiBabyItem> LostEggsFound = new List<KiwiBabyItem>(3);

	private KiwiBabyItem patrollingEgg;

	private KiwiBabyItem takeEggBackToNest;

	private float eggDist;

	public bool carryingEgg;

	public Transform grabTarget;

	private List<Transform> seenThreatsHoldingEgg = new List<Transform>();

	private IVisibleThreat attackingThreat;

	private bool attackingLastFrame;

	private bool attacking;

	public GameObject birdNest;

	public GameObject birdNestPrefab;

	public string defenseBehaviour;

	private float timeSinceSeeingThreat;

	private float checkLOSDistance;

	private Vector3 lastSeenPositionOfWatchedThreat;

	private bool checkingLastSeenPosition;

	private bool patrollingInAttackMode;

	public AudioClip[] attackSFX;

	public float hitVelocityForce = 3f;

	private float timeSinceHittingPlayer;

	public ParticleSystem rocksParticle;

	public ParticleSystem runningParticle;

	private float walkAnimSpeed = 1f;

	public AudioClip[] screamSFX;

	public AudioClip squawkSFX;

	private bool wasPatrollingEgg;

	private bool wasOwnerLastFrame;

	public AISearchRoutine searchForLostEgg;

	public GameObject playerExplodePrefab;

	private float timeSinceHittingGround;

	private int destroyTreesInterval;

	private System.Random birdRandom;

	private float attackSpeedMultiplier;

	public float chaseAccelerationMultiplier;

	private float longChaseBonusSpeed;

	public GameObject feathersPrefab;

	public AudioClip wakeUpSFX;

	public EnemyType baboonHawkType;

	public bool pryingOpenDoor;

	public HangarShipDoor shipDoor;

	private float pryingDoorAnimTime;

	public float pryOpenDoorAnimLength;

	public AudioClip breakAndEnter;

	public AudioClip shipAlarm;

	public AudioSource breakDownDoorAudio;

	private AudioSource birdNestAmbience;

	private bool wasLookingForEggs;

	private PlayerControllerB lastPlayerWhoAttacked;

	private float timeSinceGettingHit;

	private NavMeshHit navHitB;

	private float sampleNavAreaInterval;

	public PlayerControllerB abandonedThreat;

	private float timeSpentChasingThreat;

	private float timeSinceExitingAttackMode;

	private bool triedRandomDance;

	private bool targetPlayerIsInTruck;

	private float timeSinceHittingEnemy;

	ThreatType IVisibleThreat.type => ThreatType.GiantKiwi;

	int IVisibleThreat.GetThreatLevel(Vector3 seenByPosition)
	{
		if (currentBehaviourStateIndex == 2)
		{
			return 18;
		}
		return 10;
	}

	int IVisibleThreat.GetInterestLevel()
	{
		return 0;
	}

	Transform IVisibleThreat.GetThreatLookTransform()
	{
		return eye;
	}

	Transform IVisibleThreat.GetThreatTransform()
	{
		return base.transform;
	}

	Vector3 IVisibleThreat.GetThreatVelocity()
	{
		if (base.IsOwner)
		{
			return agent.velocity;
		}
		return agentLocalVelocity;
	}

	float IVisibleThreat.GetVisibility()
	{
		if (creatureAnimator.GetBool("Asleep"))
		{
			return 0.4f;
		}
		return 0.77f;
	}

	int IVisibleThreat.SendSpecialBehaviour(int id)
	{
		return 0;
	}

	GrabbableObject IVisibleThreat.GetHeldObject()
	{
		if (carryingEgg)
		{
			return takeEggBackToNest;
		}
		return null;
	}

	bool IVisibleThreat.IsThreatDead()
	{
		return isEnemyDead;
	}

	public override void Start()
	{
		base.Start();
		treeColliders = new Collider[10];
		if (base.IsServer)
		{
			SpawnBirdNest();
			SpawnNestEggs();
			syncedEggsPosition = true;
		}
		birdRandom = new System.Random(StartOfRound.Instance.randomMapSeed + 33);
		idleTimer = 15f;
		baseEyeRotationLeft = leftEyeMesh.localEulerAngles;
		baseEyeRotation = rightEyeMesh.localEulerAngles;
		shipDoor = UnityEngine.Object.FindObjectOfType<HangarShipDoor>();
	}

	private Vector3 GetRandomPositionAroundObject(Vector3 objectPos, float radius = 16f, System.Random seed = null)
	{
		return RoundManager.Instance.GetRandomNavMeshPositionInBoxPredictable(objectPos, radius, default(NavMeshHit), seed, -1, 0.5f);
	}

	[ServerRpc(RequireOwnership = false)]
	public void PeckTreeServerRpc(bool isPecking)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(1509496551u, serverRpcParams, RpcDelivery.Reliable);
				bufferWriter.WriteValueSafe(in isPecking, default(FastBufferWriter.ForPrimitives));
				__endSendServerRpc(ref bufferWriter, 1509496551u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				PeckTreeClientRpc(isPecking);
			}
		}
	}

	[ClientRpc]
	public void PeckTreeClientRpc(bool isPecking)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(2329433375u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in isPecking, default(FastBufferWriter.ForPrimitives));
			__endSendClientRpc(ref bufferWriter, 2329433375u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (!base.IsOwner)
			{
				isPeckingTree = isPecking;
				creatureAnimator.SetBool("peckTree", isPecking);
			}
		}
	}

	[ServerRpc(RequireOwnership = false)]
	public void SetSleepingServerRpc()
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(538806819u, serverRpcParams, RpcDelivery.Reliable);
				__endSendServerRpc(ref bufferWriter, 538806819u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				SetSleepingClientRpc();
			}
		}
	}

	[ClientRpc]
	public void SetSleepingClientRpc()
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(1965629606u, clientRpcParams, RpcDelivery.Reliable);
			__endSendClientRpc(ref bufferWriter, 1965629606u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (!base.IsOwner)
			{
				creatureSFX.Play();
				creatureAnimator.SetBool("Asleep", value: true);
			}
		}
	}

	public override void OnDrawGizmos()
	{
		base.OnDrawGizmos();
		if (!(HUDManager.Instance == null) && debugEnemyAI)
		{
			Gizmos.DrawWireSphere(eye.position + eye.forward * 38f + eye.up * 8f, 40f);
		}
	}

	private void LookAtTargetInterest()
	{
	}

	private bool CheckLOSForCreatures(Vector3 protectingPosition, bool protectAllEggs = false, bool mustHavePath = false)
	{
		int num = Physics.OverlapSphereNonAlloc(eye.position + eye.up * 8f, 40f, RoundManager.Instance.tempColliderResults, visibleThreatsMask, QueryTriggerInteraction.Collide);
		IVisibleThreat visibleThreat = watchingThreat;
		checkLOSDistance = 1000f;
		IVisibleThreat visibleThreat2 = null;
		IVisibleThreat visibleThreat3 = null;
		float num2 = 1000f;
		int num3 = -1;
		for (int i = 0; i < num; i++)
		{
			if (RoundManager.Instance.tempColliderResults[i] == ownCollider)
			{
				continue;
			}
			float num4 = Vector3.Distance(eye.position, RoundManager.Instance.tempColliderResults[i].transform.position);
			Vector3.Angle(RoundManager.Instance.tempColliderResults[i].transform.position - eye.position, eye.forward);
			if (!RoundManager.Instance.tempColliderResults[i].transform.TryGetComponent<IVisibleThreat>(out seenThreat))
			{
				continue;
			}
			bool flag = false;
			PlayerControllerB playerControllerB = null;
			if (seenThreat.type == ThreatType.Player)
			{
				playerControllerB = seenThreat.GetThreatTransform().GetComponent<PlayerControllerB>();
				if (abandonedThreat != null && playerControllerB == abandonedThreat)
				{
					continue;
				}
				flag = playerControllerB.isInHangarShipRoom && isInsidePlayerShip;
			}
			if (!flag && Physics.Linecast(eye.position, seenThreat.GetThreatLookTransform().position, out var _, 33556737, QueryTriggerInteraction.Ignore))
			{
				if (!debugEnemyAI)
				{
				}
				continue;
			}
			EnemyAI component = seenThreat.GetThreatTransform().GetComponent<EnemyAI>();
			if (component != null && (component.isEnemyDead || seenThreat.type == ThreatType.Bees || seenThreat.type == ThreatType.ForestGiant || (seenThreat.type == ThreatType.EyelessDog && num4 > 6f) || num4 > 16f))
			{
				continue;
			}
			float visibility = seenThreat.GetVisibility();
			if (visibility < 1f)
			{
				if (visibility == 0f)
				{
					continue;
				}
				if (visibility < 0.2f && num4 > 10f)
				{
					if (currentBehaviourStateIndex != 2 || timeSinceSeeingThreat > 0.25f)
					{
						continue;
					}
				}
				else if (visibility < 0.6f && num4 > 20f)
				{
					if (currentBehaviourStateIndex != 2 || timeSinceSeeingThreat > 0.5f)
					{
						continue;
					}
				}
				else if (visibility < 0.8f && num4 > 24f)
				{
					continue;
				}
			}
			if (debugEnemyAI)
			{
				Debug.Log($"Bird: Seeing visible threat: {RoundManager.Instance.tempColliderResults[i].transform.name}; type: {seenThreat.type}");
			}
			Transform threatTransform = seenThreat.GetThreatTransform();
			if (mustHavePath && !flag)
			{
				bool flag2 = false;
				if (i > 10 && playerControllerB == null)
				{
					if (num4 > 12f)
					{
						continue;
					}
					if (num4 < 8f)
					{
						flag2 = true;
					}
				}
				if (num4 < 1f)
				{
					flag2 = true;
				}
				if (!flag2)
				{
					Vector3 position = threatTransform.position;
					if (Physics.Raycast(threatTransform.position + Vector3.up * 0.25f, Vector3.down, out var hitInfo2, 10f, 33556737, QueryTriggerInteraction.Ignore))
					{
						position = hitInfo2.point;
						Debug.DrawRay(threatTransform.position + Vector3.up * 0.25f, Vector3.down * hitInfo2.distance, Color.blue, AIIntervalTime);
					}
					if (num4 < 30f && playerControllerB != null && playerControllerB != null && playerControllerB.isInHangarShipRoom)
					{
						position = StartOfRound.Instance.shipStrictInnerRoomBounds.ClosestPoint(position);
					}
					float sampleRadius = 2f;
					int areaMask = -1;
					VehicleController vehicleController = UnityEngine.Object.FindObjectOfType<VehicleController>();
					if (vehicleController != null && Vector3.Distance(threatTransform.position, vehicleController.transform.position) < 20f)
					{
						sampleRadius = 6f;
						areaMask = -33;
					}
					if (PathIsIntersectedByLineOfSight(RoundManager.Instance.GetNavMeshPosition(threatTransform.position, navHitB, sampleRadius, areaMask), calculatePathDistance: true, avoidLineOfSight: false) || pathDistance - num4 > 20f)
					{
						continue;
					}
				}
			}
			bool flag3 = seenThreatsHoldingEgg.Contains(threatTransform);
			if (!flag3)
			{
				for (int j = 0; j < eggs.Count; j++)
				{
					if (playerControllerB != null && eggs[j].isInShipRoom && playerControllerB.isInHangarShipRoom)
					{
						seenThreatsHoldingEgg.Add(threatTransform);
						num3 = (int)seenThreat.GetThreatTransform().GetComponent<PlayerControllerB>().playerClientId;
						flag3 = true;
						break;
					}
					// 알과의 거리가 3u 이내일때
					if (Vector3.Distance(eggs[j].transform.position, threatTransform.position) < 3f)
					{
						// 알 들고있는 플래이어에 추가
						seenThreatsHoldingEgg.Add(threatTransform);
						if (seenThreat.type == ThreatType.Player)
						{
							num3 = (int)seenThreat.GetThreatTransform().GetComponent<PlayerControllerB>().playerClientId;
						}
						flag3 = true;
						break;
					}
				}
			}
			int num5 = 1;
			if (protectAllEggs)
			{
				num5 = eggs.Count;
			}
			for (int k = 0; k < num5; k++)
			{
				if (protectAllEggs)
				{
					protectingPosition = eggs[k].transform.position;
				}
				num4 = Vector3.Distance(threatTransform.position, protectingPosition);
				if (flag3 && num4 < num2)
				{
					num2 = num4;
					visibleThreat3 = seenThreat;
				}
				if (num4 < checkLOSDistance)
				{
					checkLOSDistance = num4;
					visibleThreat2 = seenThreat;
				}
			}
		}
		if (num2 < 20f && visibleThreat3 != null)
		{
			checkLOSDistance = num2;
			visibleThreat2 = visibleThreat3;
		}
		if (num3 != -1)
		{
			AddToThreatsHoldingEggListServerRpc(num3);
		}
		if (visibleThreat2 != null)
		{
			if (watchingThreat == null || Vector3.Distance(watchingThreat.GetThreatTransform().position, protectingPosition) - checkLOSDistance > 4f)
			{
				watchingThreat = visibleThreat2;
				if (visibleThreat == null || watchingThreat != visibleThreat)
				{
					SyncWatchingThreatServerRpc(watchingThreat.GetThreatTransform().GetComponent<NetworkObject>(), (int)GameNetworkManager.Instance.localPlayerController.playerClientId);
				}
			}
			lastSeenPositionOfWatchedThreat = watchingThreat.GetThreatTransform().position;
			return true;
		}
		return false;
	}

	[ServerRpc(RequireOwnership = false)]
	public void AddToThreatsHoldingEggListServerRpc(int idOfPlayerAdded)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(2757245750u, serverRpcParams, RpcDelivery.Reliable);
				BytePacker.WriteValueBitPacked(bufferWriter, idOfPlayerAdded);
				__endSendServerRpc(ref bufferWriter, 2757245750u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				AddToThreatsHoldingEggListClientRpc(idOfPlayerAdded);
			}
		}
	}

	[ClientRpc]
	public void AddToThreatsHoldingEggListClientRpc(int idOfPlayerAdded)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(321322077u, clientRpcParams, RpcDelivery.Reliable);
			BytePacker.WriteValueBitPacked(bufferWriter, idOfPlayerAdded);
			__endSendClientRpc(ref bufferWriter, 321322077u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (!seenThreatsHoldingEgg.Contains(StartOfRound.Instance.allPlayerScripts[idOfPlayerAdded].transform))
			{
				seenThreatsHoldingEgg.Add(StartOfRound.Instance.allPlayerScripts[idOfPlayerAdded].transform);
			}
		}
	}

	[ServerRpc(RequireOwnership = false)]
	public void SyncWatchingThreatServerRpc(NetworkObjectReference seenThreatNetworkObject, int playerWhoSent)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(2000046764u, serverRpcParams, RpcDelivery.Reliable);
				bufferWriter.WriteValueSafe(in seenThreatNetworkObject, default(FastBufferWriter.ForNetworkSerializable));
				BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
				__endSendServerRpc(ref bufferWriter, 2000046764u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				SyncWatchingThreatClientRpc(seenThreatNetworkObject, playerWhoSent);
			}
		}
	}

	[ClientRpc]
	public void SyncWatchingThreatClientRpc(NetworkObjectReference seenThreatNetworkObject, int playerWhoSent)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(4160656602u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in seenThreatNetworkObject, default(FastBufferWriter.ForNetworkSerializable));
			BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
			__endSendClientRpc(ref bufferWriter, 4160656602u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (playerWhoSent != (int)GameNetworkManager.Instance.localPlayerController.playerClientId && seenThreatNetworkObject.TryGet(out var networkObject) && !networkObject.TryGetComponent<IVisibleThreat>(out watchingThreat))
			{
				Debug.LogWarning("Threat seen by bird synced across server did not have IVisibleThreat interface?");
			}
		}
	}

	private KiwiBabyItem GetClosestLostEgg(bool foundEggs = false, KiwiBabyItem excludeEgg = null, bool checkPath = false, bool allEggs = false)
	{
		float num = 2000f;
		int num2 = -1;
		if (allEggs)
		{
			for (int i = 0; i < eggs.Count; i++)
			{
				if (!eggs[i].deactivated && !eggs[i].isInFactory)
				{
					float num3 = Vector3.Distance(base.transform.position, eggs[i].transform.position);
					if (num3 < num)
					{
						num = num3;
						num2 = i;
					}
				}
			}
			if (num2 == -1)
			{
				return null;
			}
			return eggs[num2];
		}
		if (foundEggs)
		{
			for (int j = 0; j < LostEggsFound.Count; j++)
			{
				if (!LostEggsFound[j].isInFactory && !LostEggsFound[j].deactivated && (!(excludeEgg != null) || !(LostEggsFound[j] == excludeEgg)))
				{
					float num3 = Vector3.Distance(base.transform.position, LostEggsFound[j].transform.position);
					if ((!checkPath || (agent.isOnNavMesh && agent.CalculatePath(LostEggsFound[j].transform.position, path1) && !(Vector3.Distance(path1.corners[path1.corners.Length - 1], LostEggsFound[j].transform.position) > 3f))) && num3 < num)
					{
						num = num3;
						num2 = j;
					}
				}
			}
			eggDist = num;
			if (num2 == -1)
			{
				return null;
			}
			return LostEggsFound[num2];
		}
		for (int k = 0; k < LostEggs.Count; k++)
		{
			if (!LostEggs[k].isInFactory && (LostEggs[k].screaming || LostEggs[k].hasScreamed) && !LostEggs[k].deactivated && (!(excludeEgg != null) || !(LostEggs[k] == excludeEgg)))
			{
				float num3 = Vector3.Distance(base.transform.position, LostEggs[k].transform.position);
				if (num3 < num)
				{
					num = num3;
					num2 = k;
				}
			}
		}
		eggDist = num;
		if (num2 == -1)
		{
			return null;
		}
		return LostEggs[num2];
	}

	private void PickUpEgg(KiwiBabyItem egg)
	{
		if (takeEggBackToNest != null && carryingEgg)
		{
			Debug.LogError("Bird Error: GrabItemAndSync called when baboon is already carrying scrap!");
		}
		NetworkObject component = egg.GetComponent<NetworkObject>();
		GrabScrap(component);
		GrabScrapServerRpc(component, (int)GameNetworkManager.Instance.localPlayerController.playerClientId);
	}

	[ServerRpc]
	public void GrabScrapServerRpc(NetworkObjectReference item, int clientWhoSentRPC)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			if (base.OwnerClientId != networkManager.LocalClientId)
			{
				if (networkManager.LogLevel <= LogLevel.Normal)
				{
					Debug.LogError("Only the owner can invoke a ServerRpc that requires ownership!");
				}
				return;
			}
			ServerRpcParams serverRpcParams = default(ServerRpcParams);
			FastBufferWriter bufferWriter = __beginSendServerRpc(2370506911u, serverRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in item, default(FastBufferWriter.ForNetworkSerializable));
			BytePacker.WriteValueBitPacked(bufferWriter, clientWhoSentRPC);
			__endSendServerRpc(ref bufferWriter, 2370506911u, serverRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (!item.TryGet(out var networkObject))
			{
				Debug.LogError($"Baboon #{thisEnemyIndex} error: Could not get grabbed network object from reference on server");
			}
			else if ((bool)networkObject.GetComponent<GrabbableObject>() && !networkObject.GetComponent<GrabbableObject>().heldByPlayerOnServer)
			{
				GrabScrapClientRpc(item, clientWhoSentRPC);
			}
		}
	}

	[ClientRpc]
	public void GrabScrapClientRpc(NetworkObjectReference item, int clientWhoSentRPC)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(3612179633u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in item, default(FastBufferWriter.ForNetworkSerializable));
			BytePacker.WriteValueBitPacked(bufferWriter, clientWhoSentRPC);
			__endSendClientRpc(ref bufferWriter, 3612179633u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute || (!networkManager.IsClient && !networkManager.IsHost))
		{
			return;
		}
		__rpc_exec_stage = __RpcExecStage.Send;
		if (clientWhoSentRPC != (int)GameNetworkManager.Instance.localPlayerController.playerClientId)
		{
			if (item.TryGet(out var networkObject))
			{
				GrabScrap(networkObject);
			}
			else
			{
				Debug.LogError($"Baboon #{thisEnemyIndex}; Error, was not able to get id from grabbed item client rpc");
			}
		}
	}

	private void GrabScrap(NetworkObject item)
	{
		if (takeEggBackToNest != null && carryingEgg)
		{
			DropScrap(takeEggBackToNest.GetItemFloorPosition());
		}
		KiwiBabyItem component = item.gameObject.GetComponent<KiwiBabyItem>();
		carryingEgg = true;
		takeEggBackToNest = component;
		component.parentObject = grabTarget;
		component.hasHitGround = false;
		component.GrabItemFromEnemy(this);
		component.isHeldByEnemy = true;
		component.EnablePhysics(enable: false);
	}

	private void DropEgg(bool dropInNest = false)
	{
		if (takeEggBackToNest == null || !carryingEgg)
		{
			Debug.LogError("Bird Error: DropItemAndSync called when baboon has no scrap!");
		}
		NetworkObject networkObject = takeEggBackToNest.NetworkObject;
		if (!networkObject.IsSpawned)
		{
			Debug.LogError("Bird Error: Bird egg not spawned for clients");
			return;
		}
		Vector3 targetFloorPosition;
		if (dropInNest)
		{
			int num = -1;
			for (int i = 0; i < eggs.Count; i++)
			{
				if (takeEggBackToNest == eggs[i])
				{
					num = i;
					break;
				}
			}
			targetFloorPosition = birdNest.GetComponent<EnemyAINestSpawnObject>().nestPositions[num].position;
		}
		else
		{
			targetFloorPosition = takeEggBackToNest.GetItemFloorPosition();
		}
		DropScrap(targetFloorPosition, dropInNest);
		DropScrapServerRpc(networkObject, targetFloorPosition, dropInNest, (int)GameNetworkManager.Instance.localPlayerController.playerClientId);
	}

	[ServerRpc(RequireOwnership = false)]
	public void DropScrapServerRpc(NetworkObjectReference item, Vector3 targetFloorPosition, bool droppedInNest, int clientWhoSentRPC)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(3671496547u, serverRpcParams, RpcDelivery.Reliable);
				bufferWriter.WriteValueSafe(in item, default(FastBufferWriter.ForNetworkSerializable));
				bufferWriter.WriteValueSafe(in targetFloorPosition);
				bufferWriter.WriteValueSafe(in droppedInNest, default(FastBufferWriter.ForPrimitives));
				BytePacker.WriteValueBitPacked(bufferWriter, clientWhoSentRPC);
				__endSendServerRpc(ref bufferWriter, 3671496547u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				DropScrapClientRpc(item, targetFloorPosition, droppedInNest, clientWhoSentRPC);
			}
		}
	}

	[ClientRpc]
	public void DropScrapClientRpc(NetworkObjectReference item, Vector3 targetFloorPosition, bool droppedInNest, int clientWhoSentRPC)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(1775051963u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in item, default(FastBufferWriter.ForNetworkSerializable));
			bufferWriter.WriteValueSafe(in targetFloorPosition);
			bufferWriter.WriteValueSafe(in droppedInNest, default(FastBufferWriter.ForPrimitives));
			BytePacker.WriteValueBitPacked(bufferWriter, clientWhoSentRPC);
			__endSendClientRpc(ref bufferWriter, 1775051963u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute || (!networkManager.IsClient && !networkManager.IsHost))
		{
			return;
		}
		__rpc_exec_stage = __RpcExecStage.Send;
		if (clientWhoSentRPC != (int)GameNetworkManager.Instance.localPlayerController.playerClientId)
		{
			if (item.TryGet(out var _))
			{
				DropScrap(targetFloorPosition);
			}
			else
			{
				Debug.LogError("Bird: Error, was not able to get network object from dropped item client rpc");
			}
		}
	}

	private void DropScrap(Vector3 targetFloorPosition, bool droppedInNest = false)
	{
		if (takeEggBackToNest == null)
		{
			Debug.LogError("Bird: my held item is null when attempting to drop it!!");
			return;
		}
		if (takeEggBackToNest.isHeld)
		{
			takeEggBackToNest.DiscardItemFromEnemy();
			takeEggBackToNest.isHeldByEnemy = false;
			takeEggBackToNest = null;
			return;
		}
		takeEggBackToNest.parentObject = null;
		takeEggBackToNest.transform.SetParent(StartOfRound.Instance.propsContainer, worldPositionStays: true);
		takeEggBackToNest.EnablePhysics(enable: true);
		takeEggBackToNest.fallTime = 0f;
		takeEggBackToNest.startFallingPosition = takeEggBackToNest.transform.parent.InverseTransformPoint(takeEggBackToNest.transform.position);
		takeEggBackToNest.targetFloorPosition = takeEggBackToNest.transform.parent.InverseTransformPoint(targetFloorPosition);
		takeEggBackToNest.floorYRot = -1;
		takeEggBackToNest.DiscardItemFromEnemy();
		takeEggBackToNest.isHeldByEnemy = false;
		if (droppedInNest)
		{
			if (LostEggsFound.Contains(takeEggBackToNest))
			{
				LostEggsFound.Remove(takeEggBackToNest);
			}
			if (LostEggs.Contains(takeEggBackToNest))
			{
				LostEggs.Remove(takeEggBackToNest);
			}
		}
		carryingEgg = false;
		takeEggBackToNest = null;
	}

	private bool AttackIfThreatened(bool onlyWhenAwake, Vector3 protectPosition, float aggroDistance = 10f, bool forget = false)
	{
		if (onlyWhenAwake && currentBehaviourStateIndex == 0 && ((idleBehaviour == 2 && creatureAnimator.GetBool("Asleep") && miscTimer < 0.4f) || (idleBehaviour == 0 && miscTimer > 0.5f)))
		{
			return false;
		}
		if (CheckLOSForCreatures(protectPosition, protectAllEggs: false, mustHavePath: true))
		{
			float num = Vector3.Distance(watchingThreat.GetThreatTransform().position, protectPosition);
			bool flag = false;
			VehicleController vehicleController = UnityEngine.Object.FindObjectOfType<VehicleController>();
			if (vehicleController != null && Vector3.Distance(vehicleController.transform.position, protectPosition) < 20f)
			{
				flag = true;
			}
			if (flag || num < aggroDistance || seenThreatsHoldingEgg.Contains(watchingThreat.GetThreatTransform()))
			{
				if (num < 2f && !seenThreatsHoldingEgg.Contains(watchingThreat.GetThreatTransform()))
				{
					seenThreatsHoldingEgg.Add(watchingThreat.GetThreatTransform());
				}
				return true;
			}
			if (forget)
			{
				if (num < 22f)
				{
					timeSinceSeeingThreat = 0f;
				}
				else
				{
					timeSinceSeeingThreat += AIIntervalTime;
				}
			}
		}
		else if (forget)
		{
			timeSinceSeeingThreat += AIIntervalTime;
		}
		return false;
	}

	private void CarryEggBackToNest()
	{
		_ = miscTimer;
		_ = 0f;
		SetDestinationToPosition(birdNest.transform.position);
		if (Vector3.Distance(base.transform.position, birdNest.transform.position) < 3f)
		{
			DropEgg(dropInNest: true);
		}
	}

	private void PatrolAroundEgg()
	{
		if (!patrollingEgg.screaming)
		{
			behaviourTimer -= AIIntervalTime;
		}
		else
		{
			SetDestinationToPosition(patrollingEgg.transform.position);
			if (Vector3.Distance(base.transform.position, patrollingEgg.transform.position) < 14f)
			{
				behaviourTimer -= AIIntervalTime * 0.65f;
			}
		}
		if (behaviourTimer < 0f)
		{
			if (LostEggs.Contains(patrollingEgg))
			{
				LostEggs.Remove(patrollingEgg);
			}
			if (!LostEggsFound.Contains(patrollingEgg))
			{
				LostEggsFound.Add(patrollingEgg);
			}
			patrollingEgg = null;
			miscTimer = 0f;
		}
		else if (miscTimer < 0f)
		{
			miscTimer = 1.5f;
			wasPatrollingEgg = true;
			Vector3 vector = GetRandomPositionAroundObject(patrollingEgg.transform.position, 12f, idleRandom);
			if (PathIsIntersectedByLineOfSight(vector, calculatePathDistance: true, avoidLineOfSight: false) || pathDistance > 35f)
			{
				vector = birdNest.transform.position;
			}
			SetDestinationToPosition(vector);
		}
	}

	private void AbandonLostEgg(KiwiBabyItem egg)
	{
		if (LostEggs.Contains(egg))
		{
			egg.timeLastAbandoned = Time.realtimeSinceStartup;
			egg.positionWhenLastAbandoned = egg.transform.position;
			LostEggs.Remove(egg);
			if (LostEggsFound.Contains(egg))
			{
				LostEggsFound.Remove(egg);
			}
		}
	}

	private bool TryAbandonEgg(KiwiBabyItem egg, bool gotPath, Vector3 actualEggPosition)
	{
		if (!gotPath || (Vector3.Distance(base.transform.position, destination) < 16f && Vector3.Distance(destination, actualEggPosition) >= 3f))
		{
			if (!(eggDist > 20f))
			{
				AbandonLostEgg(egg);
				return true;
			}
			if (behaviourTimer < 0f)
			{
				behaviourTimer = 3f;
				AbandonLostEgg(egg);
				return true;
			}
		}
		return false;
	}

	private Vector3 GetActualEggPosition(KiwiBabyItem closestEgg)
	{
		Vector3 vector = closestEgg.transform.position;
		if (IsEggInsideClosedTruck(closestEgg))
		{
			if (Physics.Raycast(closestEgg.transform.position + Vector3.up * 0.5f, Vector3.down, out var hitInfo, 25f, 33556737, QueryTriggerInteraction.Ignore))
			{
				vector = RoundManager.Instance.GetNavMeshPosition(hitInfo.point, navHitB, 10f, -33);
			}
		}
		else if (closestEgg.isHeld && closestEgg.playerHeldBy != null)
		{
			vector = ((!Physics.Raycast(closestEgg.playerHeldBy.transform.position + Vector3.up * 0.5f, Vector3.down, out var hitInfo2, 25f, 33556737, QueryTriggerInteraction.Ignore)) ? closestEgg.playerHeldBy.transform.position : hitInfo2.point);
		}
		else if (closestEgg.isHeldByEnemy)
		{
			for (int i = 0; i < RoundManager.Instance.SpawnedEnemies.Count; i++)
			{
				if (RoundManager.Instance.SpawnedEnemies[i].enemyType.isOutsideEnemy && !RoundManager.Instance.SpawnedEnemies[i].enemyType.isDaytimeEnemy && RoundManager.Instance.SpawnedEnemies[i].enemyType == baboonHawkType)
				{
					vector = RoundManager.Instance.SpawnedEnemies[i].transform.position;
				}
			}
		}
		if (closestEgg.isInShipRoom || (closestEgg.playerHeldBy != null && closestEgg.playerHeldBy.isInHangarShipRoom))
		{
			vector = StartOfRound.Instance.shipStrictInnerRoomBounds.ClosestPoint(vector);
			vector.y = StartOfRound.Instance.playerSpawnPositions[0].position.y;
		}
		return vector;
	}

	private void BeginPryOpenDoor()
	{
		StartPryOpenDoorAnimationOnLocalClient();
		PryOpenDoorServerRpc((int)GameNetworkManager.Instance.localPlayerController.playerClientId);
	}

	private void FinishPryOpenDoor(bool cancelledEarly)
	{
		FinishPryOpenDoorAnimationOnLocalClient(cancelledEarly);
		PryOpenDoorServerRpc((int)GameNetworkManager.Instance.localPlayerController.playerClientId, finishAnim: true, cancelledEarly);
	}

	[ServerRpc(RequireOwnership = false)]
	public void PryOpenDoorServerRpc(int playerWhoSent, bool finishAnim = false, bool cancelledEarly = false)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(4019607224u, serverRpcParams, RpcDelivery.Reliable);
				BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
				bufferWriter.WriteValueSafe(in finishAnim, default(FastBufferWriter.ForPrimitives));
				bufferWriter.WriteValueSafe(in cancelledEarly, default(FastBufferWriter.ForPrimitives));
				__endSendServerRpc(ref bufferWriter, 4019607224u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				PryOpenDoorClientRpc(playerWhoSent, finishAnim, cancelledEarly);
			}
		}
	}

	[ClientRpc]
	public void PryOpenDoorClientRpc(int playerWhoSent, bool finishAnim = false, bool cancelledEarly = false)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(2152766841u, clientRpcParams, RpcDelivery.Reliable);
			BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
			bufferWriter.WriteValueSafe(in finishAnim, default(FastBufferWriter.ForPrimitives));
			bufferWriter.WriteValueSafe(in cancelledEarly, default(FastBufferWriter.ForPrimitives));
			__endSendClientRpc(ref bufferWriter, 2152766841u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute || (!networkManager.IsClient && !networkManager.IsHost))
		{
			return;
		}
		__rpc_exec_stage = __RpcExecStage.Send;
		if ((int)GameNetworkManager.Instance.localPlayerController.playerClientId != playerWhoSent)
		{
			if (!finishAnim)
			{
				StartPryOpenDoorAnimationOnLocalClient();
			}
			else
			{
				FinishPryOpenDoorAnimationOnLocalClient(cancelledEarly);
			}
		}
	}

	private void FinishPryOpenDoorAnimationOnLocalClient(bool cancelledEarly = false)
	{
		if (!cancelledEarly)
		{
			shipDoor.shipDoorsAnimator.SetBool("Closed", value: false);
			StartOfRound.Instance.SetShipDoorsClosed(closed: false);
			StartOfRound.Instance.SetShipDoorsOverheatLocalClient();
			shipDoor.doorPower = 0f;
		}
		pryingOpenDoor = false;
		inSpecialAnimation = false;
		creatureAnimator.SetBool("PryingOpenDoor", value: false);
		shipDoor.shipDoorsAnimator.SetBool("PryingOpenDoor", value: false);
		creatureAnimator.SetLayerWeight(1, 1f);
	}

	private void StartPryOpenDoorAnimationOnLocalClient()
	{
		agent.enabled = false;
		pryingOpenDoor = true;
		inSpecialAnimation = true;
		creatureAnimator.SetBool("PryingOpenDoor", value: true);
		shipDoor.shipDoorsAnimator.SetBool("PryingOpenDoor", value: true);
		shipDoor.shipDoorsAnimator.SetFloat("pryOpenDoor", 0f);
		breakDownDoorAudio.PlayOneShot(breakAndEnter);
		WalkieTalkie.TransmitOneShotAudio(breakDownDoorAudio, breakAndEnter);
		RoundManager.Instance.PlayAudibleNoise(base.transform.position, 15f, 0.9f);
		StartOfRound.Instance.speakerAudioSource.PlayOneShot(shipAlarm);
		WalkieTalkie.TransmitOneShotAudio(StartOfRound.Instance.speakerAudioSource, shipAlarm);
		if (Vector3.Distance(StartOfRound.Instance.audioListener.transform.position, base.transform.position) < 18f)
		{
			HUDManager.Instance.ShakeCamera(ScreenShakeType.VeryStrong);
		}
	}

	public bool BreakIntoShip()
	{
		if (shipDoor == null)
		{
			Debug.LogError("Bird error: ship door is null");
			return false;
		}
		if (pryingOpenDoor)
		{
			if (pryingDoorAnimTime >= 1f)
			{
				FinishPryOpenDoor(cancelledEarly: false);
			}
			return true;
		}
		if (StartOfRound.Instance.hangarDoorsClosed && StartOfRound.Instance.shipStrictInnerRoomBounds.bounds.Contains(destination) && Vector3.Distance(base.transform.position, shipDoor.outsideDoorPoint.position) < 4f)
		{
			BeginPryOpenDoor();
			return true;
		}
		return false;
	}

	private bool IsEggInsideClosedTruck(KiwiBabyItem egg, bool closedTruck = false)
	{
		VehicleController vehicleController = UnityEngine.Object.FindObjectOfType<VehicleController>();
		if (vehicleController == null)
		{
			return false;
		}
		if (egg.parentObject == vehicleController.physicsRegion.parentNetworkObject.transform)
		{
			if (closedTruck)
			{
				return !vehicleController.backDoorOpen;
			}
			return true;
		}
		return false;
	}

	[ServerRpc(RequireOwnership = false)]
	public void SetDancingServerRpc(bool setDance)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(1803512774u, serverRpcParams, RpcDelivery.Reliable);
				bufferWriter.WriteValueSafe(in setDance, default(FastBufferWriter.ForPrimitives));
				__endSendServerRpc(ref bufferWriter, 1803512774u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				SetDancingClientRpc(setDance);
			}
		}
	}

	[ClientRpc]
	public void SetDancingClientRpc(bool setDance)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(712000225u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in setDance, default(FastBufferWriter.ForPrimitives));
			__endSendClientRpc(ref bufferWriter, 712000225u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (!base.IsOwner)
			{
				creatureAnimator.SetBool("Dancing", setDance);
			}
		}
	}

	private bool HasLOSToEgg(KiwiBabyItem egg)
	{
		if (Physics.Linecast(egg.transform.position + Vector3.up * 1.6f, base.transform.position + Vector3.up * 1.6f, 256, QueryTriggerInteraction.Ignore))
		{
			if (isInsidePlayerShip)
			{
				return egg.isInShipRoom;
			}
			return false;
		}
		return true;
	}

	public bool PreoccupiedWithDefensePriorities()
	{
		agent.stoppingDistance = 1f;
		if (carryingEgg && takeEggBackToNest != null)
		{
			behaviourTimer = 3f;
			if (searchForLostEgg.inProgress)
			{
				StopSearch(searchForLostEgg);
			}
			if (AttackIfThreatened(onlyWhenAwake: false, base.transform.position, 8f))
			{
				StartAttackingAndSync();
				return true;
			}
			miscTimer -= AIIntervalTime;
			CarryEggBackToNest();
			return true;
		}
		if ((bool)patrollingEgg)
		{
			miscTimer -= AIIntervalTime;
			if (searchForLostEgg.inProgress)
			{
				StopSearch(searchForLostEgg);
			}
			if (AttackIfThreatened(onlyWhenAwake: false, patrollingEgg.transform.position, 12f))
			{
				StartAttackingAndSync();
				return true;
			}
			PatrolAroundEgg();
			return true;
		}
		if (LostEggs.Count > 0)
		{
			if (carryingEgg && takeEggBackToNest != null)
			{
				DropEgg(takeEggBackToNest);
			}
			if (wasPatrollingEgg)
			{
				wasPatrollingEgg = false;
			}
			if (!wasLookingForEggs)
			{
				wasLookingForEggs = true;
				Screech(enraged: true, sync: true);
			}
			KiwiBabyItem closestLostEgg = GetClosestLostEgg();
			if (closestLostEgg == null)
			{
				if (!searchForLostEgg.inProgress)
				{
					if (watchingThreat != null)
					{
						StartSearch(watchingThreat.GetThreatTransform().position, searchForLostEgg);
					}
					else
					{
						StartSearch(StartOfRound.Instance.shipLandingPosition.position, searchForLostEgg);
					}
				}
				if (AttackIfThreatened(onlyWhenAwake: false, base.transform.position))
				{
					StartAttackingAndSync();
					return true;
				}
				for (int i = 0; i < LostEggs.Count; i++)
				{
					if (HasLOSToEgg(LostEggs[i]) && Vector3.Distance(base.transform.position, LostEggs[i].transform.position) < 24f)
					{
						patrollingEgg = LostEggs[i];
						behaviourTimer = 3f;
						miscTimer = 1f;
						return true;
					}
				}
				return true;
			}
			if (searchForLostEgg.inProgress)
			{
				StopSearch(searchForLostEgg);
			}
			if (AttackIfThreatened(onlyWhenAwake: false, closestLostEgg.transform.position))
			{
				StartAttackingAndSync();
				return true;
			}
			Vector3 actualEggPosition = GetActualEggPosition(closestLostEgg);
			SetDestinationToPosition(actualEggPosition);
			if (closestLostEgg.isInShipRoom)
			{
				destination = StartOfRound.Instance.shipStrictInnerRoomBounds.ClosestPoint(destination);
				destination.y = StartOfRound.Instance.playerSpawnPositions[0].position.y;
			}
			bool gotPath = SetDestinationToPosition(actualEggPosition, checkForPath: true);
			if (TryAbandonEgg(closestLostEgg, gotPath, actualEggPosition))
			{
				behaviourTimer = 3f;
				return true;
			}
			behaviourTimer -= AIIntervalTime * 0.65f;
			if (HasLOSToEgg(closestLostEgg) && eggDist < 6f)
			{
				if (AttackIfThreatened(onlyWhenAwake: false, closestLostEgg.transform.position))
				{
					StartAttackingAndSync();
					return true;
				}
				patrollingEgg = closestLostEgg;
				behaviourTimer = 3f;
				miscTimer = 1f;
				closestLostEgg = GetClosestLostEgg(foundEggs: false, closestLostEgg);
				if (closestLostEgg != null && eggDist < 20f)
				{
					LostEggs.Remove(patrollingEgg);
					if (!LostEggsFound.Contains(patrollingEgg))
					{
						LostEggsFound.Add(patrollingEgg);
					}
					miscTimer = 0f;
					patrollingEgg = null;
				}
			}
			return true;
		}
		if (LostEggsFound.Count > 0)
		{
			behaviourTimer = 3f;
			if (searchForLostEgg.inProgress)
			{
				StopSearch(searchForLostEgg);
			}
			defenseBehaviour = "return eggs";
			if (takeEggBackToNest != null)
			{
				if (HasLOSToEgg(takeEggBackToNest) && Vector3.Distance(base.transform.position, takeEggBackToNest.transform.position) <= 3f)
				{
					PickUpEgg(takeEggBackToNest);
					miscTimer = 0.7f;
					return true;
				}
				if (AttackIfThreatened(onlyWhenAwake: false, takeEggBackToNest.transform.position))
				{
					StartAttackingAndSync();
					return true;
				}
			}
			else if (AttackIfThreatened(onlyWhenAwake: false, base.transform.position))
			{
				StartAttackingAndSync();
				return true;
			}
			miscTimer -= AIIntervalTime;
			if (miscTimer <= 0f)
			{
				miscTimer = 6f;
				KiwiBabyItem closestLostEgg2 = GetClosestLostEgg(foundEggs: true, null, checkPath: true);
				if (closestLostEgg2 != null)
				{
					Vector3 actualEggPosition2 = GetActualEggPosition(closestLostEgg2);
					SetDestinationToPosition(actualEggPosition2);
					if (closestLostEgg2.isInShipRoom)
					{
						destination = StartOfRound.Instance.shipStrictInnerRoomBounds.ClosestPoint(destination);
						destination.y = StartOfRound.Instance.playerSpawnPositions[0].position.y;
					}
					bool gotPath2 = SetDestinationToPosition(actualEggPosition2, checkForPath: true);
					if (TryAbandonEgg(closestLostEgg2, gotPath2, actualEggPosition2))
					{
						behaviourTimer = 3f;
						takeEggBackToNest = null;
						return true;
					}
					behaviourTimer -= AIIntervalTime * 0.65f;
					takeEggBackToNest = closestLostEgg2;
				}
			}
			return true;
		}
		return false;
	}

	private bool CheckNestForLostEgg()
	{
		bool result = false;
		if (Vector3.Distance(base.transform.position, birdNest.transform.position) < 12f && !Physics.Linecast(eye.position, birdNest.transform.position + Vector3.up * 1.2f, StartOfRound.Instance.collidersAndRoomMaskAndDefault, QueryTriggerInteraction.Ignore))
		{
			int num = 0;
			for (int i = 0; i < eggs.Count; i++)
			{
				if (Vector3.Distance(eggs[i].transform.position, birdNest.transform.position) > 6f)
				{
					num++;
				}
			}
			if (num >= 2)
			{
				for (int j = 0; j < eggs.Count; j++)
				{
					if (Vector3.Distance(eggs[j].transform.position, birdNest.transform.position) > 6f && !LostEggs.Contains(eggs[j]))
					{
						LostEggs.Add(eggs[j]);
						result = true;
					}
				}
			}
		}
		return result;
	}

	private void ReactToThreatAttack(IVisibleThreat threat, bool doLOSCheck = false)
	{
		if (threat != null && Vector3.Distance(base.transform.position, threat.GetThreatTransform().position) < 22f && (!doLOSCheck || !Physics.Linecast(eye.position, threat.GetThreatLookTransform().position, StartOfRound.Instance.collidersAndRoomMaskAndDefault, QueryTriggerInteraction.Ignore)))
		{
			if (!seenThreatsHoldingEgg.Contains(threat.GetThreatTransform()))
			{
				seenThreatsHoldingEgg.Add(threat.GetThreatTransform());
			}
			if (currentBehaviourStateIndex != 2)
			{
				watchingThreat = threat;
				attackingThreat = threat;
				StartAttackingAndSync();
			}
		}
	}

	public override void NavigateTowardsTargetPlayer()
	{
		if (setDestinationToPlayerInterval <= 0f)
		{
			setDestinationToPlayerInterval = 0.25f;
			VehicleController vehicleController = UnityEngine.Object.FindObjectOfType<VehicleController>();
			if (vehicleController != null)
			{
				Debug.Log($"dist: {Vector3.Distance(targetPlayer.transform.position, vehicleController.transform.position)}");
			}
			if (vehicleController != null && Vector3.Distance(targetPlayer.transform.position, vehicleController.transform.position) < 10f)
			{
				bool num = vehicleController.currentDriver == targetPlayer || vehicleController.currentPassenger == targetPlayer;
				bool flag = vehicleController.boundsCollider.ClosestPoint(targetPlayer.transform.position) == targetPlayer.transform.position;
				int areaMask = -1;
				if (num || (flag && !vehicleController.backDoorOpen) || vehicleController.ontopOfTruckCollider.ClosestPoint(targetPlayer.transform.position) == targetPlayer.transform.position)
				{
					targetPlayerIsInTruck = true;
					areaMask = -33;
				}
				destination = RoundManager.Instance.GetNavMeshPosition(targetPlayer.transform.position, RoundManager.Instance.navHit, 5.5f, areaMask);
			}
			else
			{
				targetPlayerIsInTruck = false;
				destination = RoundManager.Instance.GetNavMeshPosition(targetPlayer.transform.position, RoundManager.Instance.navHit, 2.7f);
			}
		}
		else
		{
			if (!targetPlayerIsInTruck)
			{
				destination = new Vector3(targetPlayer.transform.position.x, destination.y, targetPlayer.transform.position.z);
			}
			setDestinationToPlayerInterval -= Time.deltaTime;
		}
		if (addPlayerVelocityToDestination > 0f)
		{
			if (targetPlayer == GameNetworkManager.Instance.localPlayerController)
			{
				destination += Vector3.Normalize(targetPlayer.thisController.velocity * 100f) * addPlayerVelocityToDestination;
			}
			else if (targetPlayer.timeSincePlayerMoving < 0.25f)
			{
				destination += Vector3.Normalize((targetPlayer.serverPlayerPosition - targetPlayer.oldPlayerPosition) * 100f) * addPlayerVelocityToDestination;
			}
		}
	}

	public override void DoAIInterval()
	{
		base.DoAIInterval();
		if (stunNormalizedTimer > 0f)
		{
			if (stunnedByPlayer != null && stunnedByPlayer.TryGetComponent<IVisibleThreat>(out var component))
			{
				ReactToThreatAttack(component);
			}
		}
		else if (currentBehaviourStateIndex != 2 && !base.IsServer)
		{
			if (base.IsOwner)
			{
				ChangeOwnershipOfEnemy(StartOfRound.Instance.OwnerClientId);
			}
		}
		else
		{
			if (birdNest == null || StartOfRound.Instance.livingPlayers == 0)
			{
				return;
			}
			if (currentBehaviourStateIndex == 0 && (idleBehaviour != 2 || miscTimer > 0f) && CheckNestForLostEgg())
			{
				Screech(enraged: true, sync: true);
				SwitchToBehaviourState(1);
				return;
			}
			switch (currentBehaviourStateIndex)
			{
			case 0:
				if (LostEggs.Count > 0)
				{
					SwitchToBehaviourState(1);
					break;
				}
				if (searchForLostEgg.inProgress)
				{
					StopSearch(searchForLostEgg);
				}
				timeSinceExitingAttackMode += AIIntervalTime;
				abandonedThreat = null;
				agent.acceleration = 41f;
				if (AttackIfThreatened(onlyWhenAwake: true, birdNest.transform.position, 22f, forget: true))
				{
					if (checkLOSDistance < 6f)
					{
						StartAttackingAndSync();
						break;
					}
					if (UnityEngine.Random.Range(0, 100) < 50)
					{
						Screech(enraged: false, sync: true);
					}
					timeSinceSeeingThreat = 0f;
					SwitchToBehaviourState(1);
					break;
				}
				if (timeSinceSeeingThreat > 2f)
				{
					watchingThreat = null;
				}
				if (idleBehaviour == 0)
				{
					if (miscTimer > 0f)
					{
						miscTimer -= AIIntervalTime;
						agent.speed = 0f;
						break;
					}
					agent.speed = idlePatrolSpeed;
					agent.stoppingDistance = 0f;
					idleTimer -= AIIntervalTime;
					if (!triedRandomDance && Vector3.Distance(base.transform.position, destination) < 1f)
					{
						triedRandomDance = true;
						if (UnityEngine.Random.Range(0, 100) < 7)
						{
							creatureAnimator.SetBool("Dancing", value: true);
							SetDancingServerRpc(setDance: true);
						}
					}
					if (idleTimer < 0f)
					{
						idleTimer = 7f;
						Vector3 vector = GetRandomPositionAroundObject(birdNest.transform.position, 16f, idleRandom);
						if (PathIsIntersectedByLineOfSight(vector, calculatePathDistance: true, avoidLineOfSight: false) || pathDistance > 25f)
						{
							vector = birdNest.transform.position;
						}
						Debug.DrawRay(vector, Vector3.up * 5f, Color.cyan);
						SetDestinationToPosition(vector);
						triedRandomDance = false;
						creatureAnimator.SetBool("Dancing", value: false);
						SetDancingServerRpc(setDance: false);
					}
					break;
				}
				if (idleBehaviour == 1)
				{
					agent.stoppingDistance = 1f;
					agent.speed = idlePatrolSpeed;
					idleTimer -= AIIntervalTime;
					if (idleTimer < 0f || peckingTree == null)
					{
						idleTimer = 12f;
						if (isPeckingTree)
						{
							isPeckingTree = false;
							PeckTreeServerRpc(isPecking: false);
							creatureAnimator.SetBool("peckTree", value: false);
						}
						int num = Physics.OverlapSphereNonAlloc(birdNest.transform.position, 20f, treeColliders, 33554432);
						if (num <= 0)
						{
							idleBehaviour = 2;
							ChangeIdleBehaviorClientRpc(2);
							break;
						}
						int num2 = idleRandom.Next(0, num);
						peckingTree = treeColliders[num2].transform;
						SetDestinationToPosition(RoundManager.Instance.GetNavMeshPosition(peckingTree.position, navHitB));
					}
					float num3 = Vector3.Distance(base.transform.position, destination);
					if (num3 < 4f)
					{
						if (!isPeckingTree)
						{
							isPeckingTree = true;
							PeckTreeServerRpc(isPecking: true);
							creatureAnimator.SetBool("peckTree", value: true);
						}
						Vector3 vector2 = new Vector3(peckingTree.transform.position.x, base.transform.position.y, peckingTree.transform.position.z);
						SetDestinationToPosition(new Ray(vector2, base.transform.position - vector2).GetPoint(peckTreeDistance));
					}
					else if (num3 > 5f)
					{
						if (isPeckingTree)
						{
							isPeckingTree = false;
							PeckTreeServerRpc(isPecking: false);
							creatureAnimator.SetBool("peckTree", value: false);
						}
						SetDestinationToPosition(RoundManager.Instance.GetNavMeshPosition(peckingTree.position));
					}
					break;
				}
				agent.stoppingDistance = 0f;
				idleTimer -= AIIntervalTime;
				if (Vector3.Distance(base.transform.position, destination) < 2f)
				{
					miscTimer -= AIIntervalTime;
					agent.speed = 0f;
					if (miscTimer < 0.5f)
					{
						watchingThreat = null;
					}
					if (miscTimer < 0f && !creatureAnimator.GetBool("Asleep"))
					{
						creatureAnimator.SetBool("Asleep", value: true);
						creatureSFX.Play();
						SetSleepingServerRpc();
					}
				}
				else
				{
					agent.speed = idlePatrolSpeed;
				}
				break;
			case 1: 
			{
				if (BreakIntoShip())
				{
					break;
				}
				timeSinceExitingAttackMode += AIIntervalTime;
				if (abandonedThreat != null && timeSinceExitingAttackMode > 11f)
				{
					abandonedThreat = null;
				}
				float num4 = Vector3.Distance(base.transform.position, destination);
				if (num4 > 8f)
				{
					agent.speed = 18f;
					if (walkAnimSpeed < 1.5f)
					{
						walkAnimSpeed = 1.8f;
						creatureAnimator.SetFloat("WalkSpeed", walkAnimSpeed);
						SyncWalkAnimSpeedServerRpc(walkAnimSpeed);
					}
				}
				else if (num4 < 6f)
				{
					agent.speed = 12f;
					if (walkAnimSpeed > 1.5f)
					{
						walkAnimSpeed = 1f;
						creatureAnimator.SetFloat("WalkSpeed", walkAnimSpeed);
						SyncWalkAnimSpeedServerRpc(walkAnimSpeed);
					}
				}
				agent.acceleration = 41f;
				if (PreoccupiedWithDefensePriorities())
				{
					break;
				}
				if (searchForLostEgg.inProgress)
				{
					StopSearch(searchForLostEgg);
				}
				behaviourTimer = 3f;
				if (AttackIfThreatened(onlyWhenAwake: false, birdNest.transform.position, 12f, forget: true))
				{
					StartAttackingAndSync();
					break;
				}
				if (timeSinceSeeingThreat > 4f)
				{
					watchingThreat = null;
				}
				if (watchingThreat == null)
				{
					SwitchToBehaviourState(0);
					break;
				}
				agent.stoppingDistance = 4f;
				agent.acceleration = 51f;
				Ray ray = new Ray(birdNest.transform.position, watchingThreat.GetThreatTransform().position - birdNest.transform.position);
				float num5 = Vector3.Distance(watchingThreat.GetThreatTransform().position, birdNest.transform.position);
				SetDestinationToPosition(RoundManager.Instance.GetNavMeshPosition(ray.GetPoint(Mathf.Max(num5 * 0.5f, protectNestRadius)), navHitB, 8f));
				break;
			}
			case 2:
			{
				if (searchForLostEgg.inProgress)
				{
					StopSearch(searchForLostEgg);
				}
				if (!wasOwnerLastFrame)
				{
					wasOwnerLastFrame = true;
					timeSpentChasingThreat = 0f;
				}
				if (BreakIntoShip())
				{
					break;
				}
				bool flag = false;
				bool flag2 = attackingThreat.IsThreatDead();
				if (attackingThreat != null)
				{
					timeSpentChasingThreat += AIIntervalTime;
					if (flag2 && LostEggs.Count > 0)
					{
						timeSinceSeeingThreat += AIIntervalTime;
					}
					if (timeSpentChasingThreat > 12f && attackingThreat.type == ThreatType.Player && LostEggs.Count > 0)
					{
						abandonedThreat = targetPlayer;
						for (int i = 0; i < LostEggs.Count; i++)
						{
							if (LostEggs[i].isHeld && LostEggs[i].playerHeldBy != null && LostEggs[i].playerHeldBy == abandonedThreat)
							{
								AbandonLostEgg(LostEggs[i]);
							}
						}
					}
				}
				if (lastPlayerWhoAttacked != null && timeSinceGettingHit < 0.5f && Vector3.Distance(base.transform.position, lastPlayerWhoAttacked.transform.position) < 5f && !PathIsIntersectedByLineOfSight(lastPlayerWhoAttacked.transform.position, calculatePathDistance: false, avoidLineOfSight: false))
				{
					lastPlayerWhoAttacked.TryGetComponent<IVisibleThreat>(out watchingThreat);
					attackingThreat = watchingThreat;
					flag = true;
				}
				else
				{
					flag = CheckLOSForCreatures(base.transform.position, protectAllEggs: true, mustHavePath: true);
				}
				if (flag)
				{
					if (checkLOSDistance >= 17f && !seenThreatsHoldingEgg.Contains(watchingThreat.GetThreatTransform()))
					{
						timeSinceSeeingThreat += AIIntervalTime;
					}
					else
					{
						timeSinceSeeingThreat = 0f;
					}
					if (watchingThreat != attackingThreat)
					{
						timeSpentChasingThreat = 0f;
						attackingThreat = watchingThreat;
						if (watchingThreat.type == ThreatType.Player)
						{
							PlayerControllerB component2 = watchingThreat.GetThreatTransform().GetComponent<PlayerControllerB>();
							if (component2 != GameNetworkManager.Instance.localPlayerController && currentOwnershipOnThisClient == (int)GameNetworkManager.Instance.localPlayerController.playerClientId)
							{
								wasOwnerLastFrame = false;
								ChangeOwnershipOfEnemy(component2.actualClientId);
							}
							break;
						}
						if (currentOwnershipOnThisClient != 0)
						{
							targetPlayer = null;
							movingTowardsTargetPlayer = false;
							ChangeOwnershipOfEnemy(StartOfRound.Instance.OwnerClientId);
							break;
						}
					}
				}
				else
				{
					timeSinceSeeingThreat += AIIntervalTime;
				}
				if (timeSinceSeeingThreat > 4f)
				{
					SwitchToBehaviourState(1);
					ChangeOwnershipOfEnemy(StartOfRound.Instance.OwnerClientId);
					break;
				}
				if (attackingThreat != null)
				{
					bool flag3 = false;
					for (int j = 0; j < eggs.Count; j++)
					{
						if (eggs[j].screaming && Vector3.Distance(eggs[j].transform.position, attackingThreat.GetThreatTransform().position) < 3f)
						{
							flag3 = true;
							break;
						}
					}
					if (flag || flag3 || (timeSinceSeeingThreat < 0.5f && attackingThreat != null))
					{
						if (attackingThreat.type == ThreatType.Player)
						{
							targetPlayer = attackingThreat.GetThreatTransform().GetComponent<PlayerControllerB>();
							movingTowardsTargetPlayer = true;
							if (!targetPlayerIsInTruck)
							{
								addPlayerVelocityToDestination = 1f;
							}
							else
							{
								addPlayerVelocityToDestination = 0f;
							}
							agent.stoppingDistance = 0f;
						}
						else
						{
							SetDestinationToPosition(attackingThreat.GetThreatTransform().position);
						}
						checkingLastSeenPosition = true;
						miscTimer = 0f;
					}
					else
					{
						watchingThreat = null;
						if (checkingLastSeenPosition && !flag2)
						{
							if (Vector3.Distance(base.transform.position, lastSeenPositionOfWatchedThreat) > 3f)
							{
								SetDestinationToPosition(lastSeenPositionOfWatchedThreat);
							}
							else
							{
								checkingLastSeenPosition = false;
							}
						}
						else
						{
							KiwiBabyItem closestLostEgg = GetClosestLostEgg(foundEggs: false, null, checkPath: false, allEggs: true);
							if (closestLostEgg != null)
							{
								miscTimer -= AIIntervalTime;
								if (miscTimer < 0f)
								{
									miscTimer = 1.5f;
									Vector3 position = GetRandomPositionAroundObject(closestLostEgg.transform.position, 8f, birdRandom);
									if (closestLostEgg.isInShipRoom)
									{
										position = StartOfRound.Instance.shipStrictInnerRoomBounds.ClosestPoint(position);
									}
									SetDestinationToPosition(position);
								}
							}
						}
					}
				}
				agent.stoppingDistance = 0f;
				break;
			}
			}
		}
	}

	private void SpawnExplosionAtDeadBodyAndSync()
	{
		if (base.IsOwner)
		{
			targetPlayer.deadBody.DeactivateBody(setActive: false);
			UnityEngine.Object.Instantiate(playerExplodePrefab, targetPlayer.deadBody.transform.position, targetPlayer.transform.rotation, RoundManager.Instance.mapPropsContainer.transform);
			int deactivatePlayerBody = (int)targetPlayer.playerClientId;
			SpawnExplosionAtPlayerBodyServerRpc(targetPlayer.deadBody.transform.position, targetPlayer.transform.rotation, (int)GameNetworkManager.Instance.localPlayerController.playerClientId, deactivatePlayerBody);
		}
	}

	[ServerRpc(RequireOwnership = false)]
	public void SpawnExplosionAtPlayerBodyServerRpc(Vector3 pos, Quaternion rot, int playerWhoSent, int deactivatePlayerBody)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(3238921108u, serverRpcParams, RpcDelivery.Reliable);
				bufferWriter.WriteValueSafe(in pos);
				bufferWriter.WriteValueSafe(in rot);
				BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
				BytePacker.WriteValueBitPacked(bufferWriter, deactivatePlayerBody);
				__endSendServerRpc(ref bufferWriter, 3238921108u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				SpawnExplosionAtPlayerBodyClientRpc(pos, rot, playerWhoSent, deactivatePlayerBody);
			}
		}
	}

	[ClientRpc]
	public void SpawnExplosionAtPlayerBodyClientRpc(Vector3 pos, Quaternion rot, int playerWhoSent, int deactivatePlayerBody)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(1143323347u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in pos);
			bufferWriter.WriteValueSafe(in rot);
			BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
			BytePacker.WriteValueBitPacked(bufferWriter, deactivatePlayerBody);
			__endSendClientRpc(ref bufferWriter, 1143323347u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute || (!networkManager.IsClient && !networkManager.IsHost))
		{
			return;
		}
		__rpc_exec_stage = __RpcExecStage.Send;
		if ((int)GameNetworkManager.Instance.localPlayerController.playerClientId != playerWhoSent)
		{
			targetPlayer.deadBody.DeactivateBody(setActive: false);
			UnityEngine.Object.Instantiate(playerExplodePrefab, pos, rot, RoundManager.Instance.mapPropsContainer.transform);
			if (StartOfRound.Instance.allPlayerScripts[deactivatePlayerBody].deadBody != null)
			{
				StartOfRound.Instance.allPlayerScripts[deactivatePlayerBody].deadBody.DeactivateBody(setActive: false);
			}
		}
	}

	[ServerRpc(RequireOwnership = false)]
	public void SyncWalkAnimSpeedServerRpc(float speed)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(4085505192u, serverRpcParams, RpcDelivery.Reliable);
				bufferWriter.WriteValueSafe(in speed, default(FastBufferWriter.ForPrimitives));
				__endSendServerRpc(ref bufferWriter, 4085505192u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				SyncWalkAnimSpeedClientRpc(speed);
			}
		}
	}

	[ClientRpc]
	public void SyncWalkAnimSpeedClientRpc(float speed)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(2658683365u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in speed, default(FastBufferWriter.ForPrimitives));
			__endSendClientRpc(ref bufferWriter, 2658683365u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (!base.IsOwner)
			{
				walkAnimSpeed = speed;
			}
		}
	}

	private void StartAttackingAndSync()
	{
		if (watchingThreat == null)
		{
			Debug.LogError("StartAttackingAndSync called with no watchingThreat currently set");
			return;
		}
		if (watchingThreat.type == ThreatType.Player)
		{
			targetPlayer = watchingThreat.GetThreatTransform().gameObject.GetComponent<PlayerControllerB>();
			currentOwnershipOnThisClient = (int)targetPlayer.playerClientId;
		}
		else
		{
			targetPlayer = null;
			currentOwnershipOnThisClient = (int)StartOfRound.Instance.allPlayerScripts[0].playerClientId;
		}
		attackingThreat = watchingThreat;
		if (base.IsServer)
		{
			if (targetPlayer != null)
			{
				if (base.gameObject.GetComponent<NetworkObject>().OwnerClientId != targetPlayer.actualClientId)
				{
					thisNetworkObject.ChangeOwnership(targetPlayer.actualClientId);
				}
			}
			else if (base.gameObject.GetComponent<NetworkObject>().OwnerClientId != StartOfRound.Instance.allPlayerScripts[0].actualClientId)
			{
				thisNetworkObject.ChangeOwnership(StartOfRound.Instance.allPlayerScripts[0].actualClientId);
			}
		}
		Screech();
		timeSpentChasingThreat = 0f;
		SwitchToBehaviourStateOnLocalClient(2);
		List<int> list = new List<int>();
		for (int i = 0; i < StartOfRound.Instance.allPlayerScripts.Length; i++)
		{
			if (seenThreatsHoldingEgg.Contains(StartOfRound.Instance.allPlayerScripts[i].transform))
			{
				list.Add((int)StartOfRound.Instance.allPlayerScripts[i].playerClientId);
			}
		}
		int abandonedThreatInt = -1;
		if (abandonedThreat != null)
		{
			abandonedThreatInt = (int)abandonedThreat.playerClientId;
		}
		StartAttackingThreatServerRpc(attackingThreat.GetThreatTransform().gameObject.GetComponent<NetworkObject>(), (int)GameNetworkManager.Instance.localPlayerController.playerClientId, list.ToArray(), abandonedThreatInt);
	}

	private void AddPlayerIDsToSeenThreatsHoldingEggsList(int[] IDs)
	{
		for (int i = 0; i < IDs.Length; i++)
		{
			if (!seenThreatsHoldingEgg.Contains(StartOfRound.Instance.allPlayerScripts[IDs[i]].transform))
			{
				seenThreatsHoldingEgg.Add(StartOfRound.Instance.allPlayerScripts[IDs[i]].transform);
			}
		}
	}

	[ServerRpc(RequireOwnership = false)]
	public void StartAttackingThreatServerRpc(NetworkObjectReference threatObjectRef, int playerWhoSent, int[] seenThreatsHoldingEggsPlayerIDs, int abandonedThreatInt)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			ServerRpcParams serverRpcParams = default(ServerRpcParams);
			FastBufferWriter bufferWriter = __beginSendServerRpc(639546879u, serverRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in threatObjectRef, default(FastBufferWriter.ForNetworkSerializable));
			BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
			bool value = seenThreatsHoldingEggsPlayerIDs != null;
			bufferWriter.WriteValueSafe(in value, default(FastBufferWriter.ForPrimitives));
			if (value)
			{
				bufferWriter.WriteValueSafe(seenThreatsHoldingEggsPlayerIDs, default(FastBufferWriter.ForPrimitives));
			}
			BytePacker.WriteValueBitPacked(bufferWriter, abandonedThreatInt);
			__endSendServerRpc(ref bufferWriter, 639546879u, serverRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute || (!networkManager.IsServer && !networkManager.IsHost))
		{
			return;
		}
		__rpc_exec_stage = __RpcExecStage.Send;
		if (!threatObjectRef.TryGet(out var networkObject))
		{
			Debug.LogError($"Bird error: Unable to get network object out of synced net object ref for StartAttackingServerRpc; id: {threatObjectRef.NetworkObjectId}");
			return;
		}
		if (!networkObject.gameObject.TryGetComponent<IVisibleThreat>(out attackingThreat))
		{
			Debug.LogError("Bird error: Unable to get IVisibleThreat interface out of synced network object ref for StartAttackingServerRpc; object: '" + networkObject.gameObject.name + "'");
			return;
		}
		ulong num;
		int value2;
		if (attackingThreat.type != ThreatType.Player)
		{
			num = StartOfRound.Instance.OwnerClientId;
			if (!StartOfRound.Instance.ClientPlayerList.TryGetValue(num, out value2))
			{
				Debug.LogError($"Bird: Unable to get player value from clientplayerlist for client id: {num} which should be host.");
			}
			targetPlayer = null;
		}
		else
		{
			PlayerControllerB component = attackingThreat.GetThreatTransform().GetComponent<PlayerControllerB>();
			num = component.actualClientId;
			targetPlayer = component;
			if (!StartOfRound.Instance.ClientPlayerList.TryGetValue(num, out value2))
			{
				Debug.LogError($"Bird: Unable to get player value from clientplayerlist for client id: {num}");
				targetPlayer = null;
			}
		}
		if (base.gameObject.GetComponent<NetworkObject>().OwnerClientId != num)
		{
			thisNetworkObject.ChangeOwnership(num);
		}
		currentOwnershipOnThisClient = value2;
		if (playerWhoSent != (int)GameNetworkManager.Instance.localPlayerController.playerClientId)
		{
			Screech();
			timeSpentChasingThreat = 0f;
			AddPlayerIDsToSeenThreatsHoldingEggsList(seenThreatsHoldingEggsPlayerIDs);
			if (abandonedThreatInt != -1)
			{
				abandonedThreat = StartOfRound.Instance.allPlayerScripts[abandonedThreatInt];
			}
		}
		StartAttackingClientRpc(attackingThreat.GetThreatTransform().gameObject.GetComponent<NetworkObject>(), value2, playerWhoSent, seenThreatsHoldingEggsPlayerIDs, abandonedThreatInt);
	}

	[ClientRpc]
	public void StartAttackingClientRpc(NetworkObjectReference threat, int playerVal, int playerWhoSent, int[] seenThreatsHoldingEggsPlayerIDs, int abandonedThreatInt)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(2397901577u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in threat, default(FastBufferWriter.ForNetworkSerializable));
			BytePacker.WriteValueBitPacked(bufferWriter, playerVal);
			BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
			bool value = seenThreatsHoldingEggsPlayerIDs != null;
			bufferWriter.WriteValueSafe(in value, default(FastBufferWriter.ForPrimitives));
			if (value)
			{
				bufferWriter.WriteValueSafe(seenThreatsHoldingEggsPlayerIDs, default(FastBufferWriter.ForPrimitives));
			}
			BytePacker.WriteValueBitPacked(bufferWriter, abandonedThreatInt);
			__endSendClientRpc(ref bufferWriter, 2397901577u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute || (!networkManager.IsClient && !networkManager.IsHost))
		{
			return;
		}
		__rpc_exec_stage = __RpcExecStage.Send;
		if ((int)GameNetworkManager.Instance.localPlayerController.playerClientId == playerWhoSent)
		{
			return;
		}
		if (threat.TryGet(out var networkObject))
		{
			if (!networkObject.gameObject.TryGetComponent<IVisibleThreat>(out var component))
			{
				Debug.LogError("Bird Error: StartAttackingClientRpc - Could not get an IVisibleThreat interface from synced NetworkObject '" + networkObject.gameObject.name + "'");
			}
			else
			{
				if (component.type == ThreatType.Player)
				{
					targetPlayer = StartOfRound.Instance.allPlayerScripts[playerVal];
				}
				else
				{
					targetPlayer = null;
				}
				attackingThreat = component;
				watchingThreat = component;
			}
		}
		Screech();
		timeSpentChasingThreat = 0f;
		if (abandonedThreatInt != -1)
		{
			abandonedThreat = StartOfRound.Instance.allPlayerScripts[abandonedThreatInt];
		}
		currentOwnershipOnThisClient = playerVal;
		SwitchToBehaviourStateOnLocalClient(2);
		AddPlayerIDsToSeenThreatsHoldingEggsList(seenThreatsHoldingEggsPlayerIDs);
	}

	public override void Update()
	{
		base.Update();
		if (watchingThreat != null)
		{
			watchingThreatTransform = watchingThreat.GetThreatTransform();
		}
		else
		{
			watchingThreatTransform = null;
		}
		if (isEnemyDead)
		{
			creatureAnimator.SetLayerWeight(1, Mathf.Max(0f, creatureAnimator.GetLayerWeight(1) - Time.deltaTime * 5f));
		}
		else
		{
			if (inKillAnimation)
			{
				return;
			}
			if (birdNest == null)
			{
				if (base.IsServer)
				{
					return;
				}
				EnemyAINestSpawnObject[] array = UnityEngine.Object.FindObjectsByType<EnemyAINestSpawnObject>(FindObjectsSortMode.None);
				for (int i = 0; i < array.Length; i++)
				{
					if (array[i].enemyType == enemyType)
					{
						birdNest = array[i].gameObject;
						birdNestAmbience = array[i].GetComponent<AudioSource>();
						break;
					}
				}
				return;
			}
			creatureAnimator.SetBool("Stunned", stunNormalizedTimer > 0f);
			if (pryingOpenDoor && inSpecialAnimation)
			{
				base.transform.position = Vector3.Lerp(base.transform.position, shipDoor.outsideDoorPoint.position, 7f * Time.deltaTime);
				base.transform.rotation = Quaternion.Lerp(base.transform.rotation, shipDoor.outsideDoorPoint.rotation, 7f * Time.deltaTime);
				pryingDoorAnimTime = Mathf.Min(pryingDoorAnimTime + Time.deltaTime / pryOpenDoorAnimLength, 1f);
				creatureAnimator.SetFloat("pryOpenDoor", pryingDoorAnimTime);
				shipDoor.shipDoorsAnimator.SetFloat("pryOpenDoor", pryingDoorAnimTime);
				creatureAnimator.SetLayerWeight(1, Mathf.Max(0f, creatureAnimator.GetLayerWeight(1) - Time.deltaTime * 5f));
				if (pryingDoorAnimTime > 0.12f)
				{
					EnableEnemyMesh(enable: true);
				}
				BreakIntoShip();
				return;
			}
			if (stunNormalizedTimer > 0f)
			{
				if (base.IsOwner)
				{
					agent.speed = 0f;
					agent.acceleration = 5000f;
				}
				creatureAnimator.SetBool("Attacking", value: false);
				return;
			}
			pingAttentionTimer -= Time.deltaTime;
			timeSinceHittingPlayer += Time.deltaTime;
			timeSincePingingAttention += Time.deltaTime;
			timeSinceHittingGround += Time.deltaTime;
			timeSinceGettingHit += Time.deltaTime;
			CalculateAnimationDirection(1.2f);
			CalculateLookingAnimation();
			creatureAnimator.SetBool("holdEgg", carryingEgg);
			for (int j = 0; j < eggs.Count; j++)
			{
				if (!LostEggs.Contains(eggs[j]) && eggs[j].screaming && (!(Time.realtimeSinceStartup - eggs[j].timeLastAbandoned < 25f) || (!(Vector3.Distance(eggs[j].transform.position, eggs[j].positionWhenLastAbandoned) < 10f) && (!(abandonedThreat != null) || !eggs[j].isHeld || !(eggs[j].playerHeldBy != null) || !(eggs[j].playerHeldBy == abandonedThreat)))))
				{
					LostEggs.Add(eggs[j]);
					if (LostEggsFound.Contains(eggs[j]))
					{
						LostEggsFound.Remove(eggs[j]);
					}
				}
			}
			if (!base.IsOwner)
			{
				wasOwnerLastFrame = false;
			}
			switch (currentBehaviourStateIndex)
			{
			case 0:
				if (currentBehaviourStateIndex != previousBehaviour)
				{
					idleBehaviour = 0;
					wasLookingForEggs = false;
					creatureAnimator.SetBool("Running", value: false);
					agent.speed = idlePatrolSpeed;
					behaviourTimer = 15f;
					idleTimer = 4f;
					previousBehaviour = currentBehaviourStateIndex;
				}
				if (previousIdleBehaviour != idleBehaviour)
				{
					if (idleBehaviour != 2 && birdNestAmbience.isPlaying)
					{
						birdNestAmbience.Stop();
					}
					if (idleBehaviour == 0 && previousIdleBehaviour == 2)
					{
						if (creatureSFX.isPlaying)
						{
							creatureSFX.Stop();
						}
						creatureSFX.PlayOneShot(wakeUpSFX);
						miscTimer = 2f;
					}
					else
					{
						miscTimer = 0f;
					}
					if (idleBehaviour != 1)
					{
						creatureAnimator.SetBool("peckTree", value: false);
						isPeckingTree = false;
						peckingTree = null;
					}
					if (idleBehaviour != 2)
					{
						creatureAnimator.SetBool("Asleep", value: false);
					}
					if (idleBehaviour != 0)
					{
						creatureAnimator.SetBool("Dancing", value: false);
					}
					idleRandom = new System.Random(StartOfRound.Instance.randomMapSeed + 33 + idleBehaviour);
					if (idleBehaviour == 2)
					{
						miscTimer = 1.5f;
						if (base.IsOwner)
						{
							Vector3 vector = GetRandomPositionAroundObject(birdNest.transform.position, 5f, idleRandom);
							if (PathIsIntersectedByLineOfSight(vector, calculatePathDistance: true, avoidLineOfSight: false) || pathDistance > 35f)
							{
								vector = birdNest.transform.position;
							}
							Debug.DrawRay(vector, Vector3.up * 5f, Color.green);
							SetDestinationToPosition(vector);
						}
					}
					attacking = false;
					creatureAnimator.SetBool("Attacking", value: false);
					previousIdleBehaviour = idleBehaviour;
				}
				if (idleBehaviour != 0)
				{
					if (idleBehaviour == 1)
					{
						if (isPeckingTree && peckingTree != null && base.IsOwner)
						{
							RoundManager.Instance.tempTransform.position = base.transform.position;
							RoundManager.Instance.tempTransform.LookAt(peckingTree.transform.position);
							RoundManager.Instance.tempTransform.eulerAngles = new Vector3(0f, RoundManager.Instance.tempTransform.eulerAngles.y, 0f);
							base.transform.rotation = Quaternion.Lerp(base.transform.rotation, RoundManager.Instance.tempTransform.rotation, 10f * Time.deltaTime);
						}
					}
					else
					{
						if (!birdNestAmbience.isPlaying)
						{
							birdNestAmbience.Play();
						}
						birdNestAmbience.volume = Mathf.Min(birdNestAmbience.volume + Time.deltaTime * 0.5f, 1f);
					}
				}
				if (base.IsServer)
				{
					if (idleBehaviour == 2)
					{
						behaviourTimer -= Time.deltaTime * 0.82f;
					}
					else
					{
						behaviourTimer -= Time.deltaTime;
					}
					if (behaviourTimer < 0f)
					{
						behaviourTimer = 18f;
						idleBehaviour = (idleBehaviour + 1) % 3;
						ChangeIdleBehaviorClientRpc(idleBehaviour);
					}
				}
				creatureAnimator.SetFloat("WalkSpeed", 1f);
				break;
			case 1:
				if (currentBehaviourStateIndex != previousBehaviour)
				{
					creatureAnimator.SetBool("peckTree", value: false);
					isPeckingTree = false;
					if (creatureSFX.isPlaying)
					{
						creatureSFX.Stop();
					}
					if (birdNestAmbience.isPlaying)
					{
						birdNestAmbience.Stop();
					}
					creatureAnimator.SetBool("Asleep", value: false);
					creatureAnimator.SetBool("Dancing", value: false);
					agent.speed = 12f;
					attacking = false;
					creatureAnimator.SetBool("Attacking", value: false);
					previousBehaviour = currentBehaviourStateIndex;
					timeSinceSeeingThreat = 0f;
				}
				if (base.IsOwner && carryingEgg && takeEggBackToNest != null && miscTimer > 0f)
				{
					agent.speed = 0f;
				}
				if (!base.IsOwner)
				{
					creatureAnimator.SetFloat("WalkSpeed", walkAnimSpeed);
					creatureAnimator.SetBool("Running", velZ > 0.5f && velX < 0.4f && Vector3.Distance(base.transform.position, destination) > 5f);
				}
				break;
			case 2:
				if (currentBehaviourStateIndex != previousBehaviour)
				{
					creatureAnimator.SetBool("PeckTree", value: false);
					isPeckingTree = false;
					if (creatureSFX.isPlaying)
					{
						creatureSFX.Stop();
					}
					creatureAnimator.SetBool("Asleep", value: false);
					creatureAnimator.SetBool("Dancing", value: false);
					timeSinceSeeingThreat = 0f;
					timeSinceExitingAttackMode = 0f;
					if (birdNestAmbience.isPlaying)
					{
						birdNestAmbience.Stop();
					}
					patrollingInAttackMode = false;
					checkingLastSeenPosition = true;
					miscTimer = 0f;
					attackSpeedMultiplier = 0.45f;
					longChaseBonusSpeed = 0f;
					behaviourTimer = 0.8f;
					if (base.IsServer && carryingEgg && takeEggBackToNest != null)
					{
						DropEgg();
					}
					previousBehaviour = currentBehaviourStateIndex;
				}
				behaviourTimer -= Time.deltaTime;
				if (behaviourTimer > 0f)
				{
					attacking = false;
					creatureAnimator.SetFloat("WalkSpeed", 0f);
					creatureAnimator.SetBool("Attacking", value: false);
					if (base.IsOwner)
					{
						agent.speed = 0f;
					}
					break;
				}
				attackSpeedMultiplier = Mathf.Min(1.4f, attackSpeedMultiplier + Time.deltaTime * chaseAccelerationMultiplier);
				if (attackSpeedMultiplier > 0.85f)
				{
					agent.acceleration = Mathf.Min(140f, agent.acceleration + Time.deltaTime * 2f);
				}
				if (attackSpeedMultiplier >= 1.1f)
				{
					longChaseBonusSpeed += Time.deltaTime * 0.5f;
				}
				else if (attackSpeedMultiplier <= 0.85f)
				{
					agent.acceleration = Mathf.Max(45f, agent.acceleration - Time.deltaTime * 3f);
					longChaseBonusSpeed = Mathf.Clamp(longChaseBonusSpeed - Time.deltaTime * 2f, 6.3f, 40f);
				}
				if (base.IsOwner)
				{
					agent.speed = 10f * (attackSpeedMultiplier * 1.4f) + longChaseBonusSpeed;
				}
				if (watchingThreat != null)
				{
					attacking = Vector3.Distance(base.transform.position, watchingThreat.GetThreatTransform().position) < 10f;
					if (!attacking)
					{
						if (Physics.Linecast(eye.position, watchingThreat.GetThreatLookTransform().position, StartOfRound.Instance.collidersAndRoomMaskAndDefault, QueryTriggerInteraction.Ignore))
						{
							attackSpeedMultiplier = Mathf.Max(0.7f, attackSpeedMultiplier - Time.deltaTime * (chaseAccelerationMultiplier * 1.5f));
						}
						if (watchingThreat.type == ThreatType.Player)
						{
							PlayerControllerB component = watchingThreat.GetThreatTransform().GetComponent<PlayerControllerB>();
							if (component.deadBody != null && !component.deadBody.deactivated && Vector3.Distance(base.transform.position, component.deadBody.bodyParts[0].transform.position) < 5f)
							{
								attacking = true;
							}
						}
					}
					if (base.IsOwner && movingTowardsTargetPlayer && targetPlayer != null && targetPlayer.isInHangarShipRoom)
					{
						destination = StartOfRound.Instance.shipStrictInnerRoomBounds.ClosestPoint(destination);
						destination.y = StartOfRound.Instance.playerSpawnPositions[0].position.y;
					}
				}
				else
				{
					attacking = false;
				}
				creatureAnimator.SetFloat("attackSpeed", attackSpeedMultiplier);
				creatureAnimator.SetBool("Attacking", attacking);
				creatureAnimator.SetFloat("WalkSpeed", attackSpeedMultiplier + 1.2f);
				break;
			}
			sampleNavAreaInterval -= Time.deltaTime;
			if (sampleNavAreaInterval < 0f)
			{
				sampleNavAreaInterval = 0.2f;
				if (!creatureAnimator.GetBool("Crawling"))
				{
					if (!isInsidePlayerShip && !pryingOpenDoor && Physics.Raycast(base.transform.position + Vector3.up * 0.2f, Vector3.up, 4.55f, StartOfRound.Instance.collidersAndRoomMaskAndDefault, QueryTriggerInteraction.Ignore))
					{
						creatureAnimator.SetBool("Crawling", value: true);
						agent.acceleration = 500f;
					}
				}
				else if (isInsidePlayerShip || pryingOpenDoor || !Physics.Raycast(base.transform.position + Vector3.up * 0.2f, Vector3.up, 4.55f, StartOfRound.Instance.collidersAndRoomMaskAndDefault, QueryTriggerInteraction.Ignore))
				{
					creatureAnimator.SetBool("Crawling", value: false);
					agent.acceleration = 50f;
				}
			}
			if (base.IsOwner && creatureAnimator.GetBool("Crawling"))
			{
				agent.speed = Mathf.Min(agent.speed, 4f);
			}
		}
	}

	[ClientRpc]
	public void ChangeIdleBehaviorClientRpc(int newIdleBehavior)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(3575423690u, clientRpcParams, RpcDelivery.Reliable);
			BytePacker.WriteValueBitPacked(bufferWriter, newIdleBehavior);
			__endSendClientRpc(ref bufferWriter, 3575423690u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (!base.IsServer)
			{
				idleBehaviour = newIdleBehavior;
			}
		}
	}

	private bool CheckPathFromNodeToShip(Transform node)
	{
		NavMeshPath navMeshPath = new NavMeshPath();
		if (Physics.Raycast(node.transform.position, Vector3.down, out var hitInfo, 5f, StartOfRound.Instance.collidersAndRoomMaskAndDefault, QueryTriggerInteraction.Ignore) && NavMesh.CalculatePath(hitInfo.point, StartOfRound.Instance.outsideDoorPosition.position, -1, navMeshPath) && navMeshPath.status == NavMeshPathStatus.PathComplete)
		{
			return true;
		}
		return false;
	}

	private void SpawnBirdNest()
	{
		GameObject[] array = (from x in GameObject.FindGameObjectsWithTag("OutsideAINode")
			orderby Vector3.Distance(x.transform.position, StartOfRound.Instance.shipLandingPosition.transform.position) descending
			select x).ToArray();
		Vector3 position = array[0].transform.position;
		System.Random random = new System.Random(StartOfRound.Instance.randomMapSeed + 288);
		int num = random.Next(0, array.Length / 3);
		bool flag = false;
		Vector3 vector = Vector3.zero;
		for (int num2 = 0; num2 < array.Length; num2++)
		{
			position = array[num].transform.position;
			position = RoundManager.Instance.GetRandomNavMeshPositionInBoxPredictable(position, 15f, default(NavMeshHit), random, RoundManager.Instance.GetLayermaskForEnemySizeLimit(enemyType));
			position = RoundManager.Instance.PositionWithDenialPointsChecked(position, array, enemyType, enemyType.nestDistanceFromShip);
			flag = CheckPathFromNodeToShip(array[num].transform);
			if (flag)
			{
				vector = RoundManager.Instance.PositionEdgeCheck(position, enemyType.nestSpawnPrefabWidth);
			}
			if (vector == Vector3.zero || !flag)
			{
				num++;
				if (num > array.Length - 1)
				{
					position = RoundManager.Instance.GetRandomNavMeshPositionInBoxPredictable(array[0].transform.position, 15f, default(NavMeshHit), random, RoundManager.Instance.GetLayermaskForEnemySizeLimit(enemyType));
					break;
				}
				continue;
			}
			position = vector;
			break;
		}
		GameObject gameObject = UnityEngine.Object.Instantiate(birdNestPrefab, position, Quaternion.Euler(Vector3.zero), RoundManager.Instance.mapPropsContainer.transform);
		gameObject.transform.Rotate(Vector3.up, random.Next(-180, 180), Space.World);
		if (!gameObject.gameObject.GetComponentInChildren<NetworkObject>())
		{
			Debug.LogError("Error: No NetworkObject found in enemy nest spawn prefab that was just spawned on the host: '" + gameObject.name + "'");
		}
		else
		{
			gameObject.gameObject.GetComponentInChildren<NetworkObject>().Spawn(destroyWithScene: true);
		}
		birdNestAmbience = gameObject.GetComponent<AudioSource>();
		birdNest = gameObject;
		agent.enabled = false;
		base.transform.position = birdNest.transform.position;
		base.transform.rotation = birdNest.transform.rotation;
		agent.enabled = true;
	}

	private void SpawnNestEggs()
	{
		if (!base.IsServer)
		{
			return;
		}
		if (birdNest == null)
		{
			Debug.LogError($"{enemyType.enemyName} #{thisEnemyIndex}: Nest object is null!");
			return;
		}
		nestEggSpawnPositions = birdNest.GetComponent<EnemyAINestSpawnObject>().nestPositions;
		System.Random random = new System.Random(StartOfRound.Instance.randomMapSeed + 1316 + enemyType.numberSpawned);
		NetworkObjectReference[] array = new NetworkObjectReference[3];
		int[] array2 = new int[3];
		for (int i = 0; i < 3; i++)
		{
			GameObject obj = UnityEngine.Object.Instantiate(eggPrefab, nestEggSpawnPositions[i].position + Vector3.up * 0.2f, Quaternion.Euler(Vector3.zero), RoundManager.Instance.spawnedScrapContainer);
			obj.SetActive(value: true);
			NetworkObject component = obj.GetComponent<NetworkObject>();
			component.Spawn();
			array[i] = component;
			int num = ((!(Vector3.Distance(birdNest.transform.position, StartOfRound.Instance.shipLandingPosition.transform.position) > 100f)) ? random.Next(40, 70) : ((UnityEngine.Random.Range(0, 100) >= 30) ? random.Next(70, 120) : random.Next(70, 200)));
			array2[i] = num;
		}
		Vector3[] array3 = new Vector3[3];
		for (int j = 0; j < array3.Length; j++)
		{
			array3[j] = nestEggSpawnPositions[j].position;
		}
		SpawnEggsClientRpc(array, array2, array3);
	}

	[ClientRpc]
	public void SpawnEggsClientRpc(NetworkObjectReference[] eggNetworkReferences, int[] eggScrapValues, Vector3[] nestSpawnPositions)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(276418090u, clientRpcParams, RpcDelivery.Reliable);
			bool value = eggNetworkReferences != null;
			bufferWriter.WriteValueSafe(in value, default(FastBufferWriter.ForPrimitives));
			if (value)
			{
				bufferWriter.WriteValueSafe(eggNetworkReferences, default(FastBufferWriter.ForNetworkSerializable));
			}
			bool value2 = eggScrapValues != null;
			bufferWriter.WriteValueSafe(in value2, default(FastBufferWriter.ForPrimitives));
			if (value2)
			{
				bufferWriter.WriteValueSafe(eggScrapValues, default(FastBufferWriter.ForPrimitives));
			}
			bool value3 = nestSpawnPositions != null;
			bufferWriter.WriteValueSafe(in value3, default(FastBufferWriter.ForPrimitives));
			if (value3)
			{
				bufferWriter.WriteValueSafe(nestSpawnPositions);
			}
			__endSendClientRpc(ref bufferWriter, 276418090u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute || (!networkManager.IsClient && !networkManager.IsHost))
		{
			return;
		}
		__rpc_exec_stage = __RpcExecStage.Send;
		for (int i = 0; i < 3; i++)
		{
			if (eggNetworkReferences[i].TryGet(out var networkObject))
			{
				KiwiBabyItem component = networkObject.gameObject.GetComponent<KiwiBabyItem>();
				eggs.Add(component);
				component.scrapValue = eggScrapValues[i];
				ScanNodeProperties componentInChildren = component.gameObject.GetComponentInChildren<ScanNodeProperties>();
				if (componentInChildren != null)
				{
					componentInChildren.scrapValue = eggScrapValues[i];
					componentInChildren.headerText = "Egg";
					componentInChildren.subText = $"VALUE: ${eggScrapValues[i]}";
				}
				component.targetFloorPosition = nestSpawnPositions[i] + Vector3.up * 0.2f;
				RoundManager.Instance.totalScrapValueInLevel += eggScrapValues[i];
				hasSpawnedEggs = true;
			}
			else
			{
				Debug.LogError("Bird: Error! Egg could not be accessed from network object reference");
			}
		}
	}

	public void Screech(bool enraged = true, bool sync = false)
	{
		float num = Vector3.Distance(StartOfRound.Instance.audioListener.transform.position, base.transform.position);
		if (enraged)
		{
			creatureAnimator.SetTrigger("ScreamEnraged");
			RoundManager.PlayRandomClip(creatureVoice, screamSFX, randomize: true, 1f, 10111);
			WalkieTalkie.TransmitOneShotAudio(creatureSFX, screamSFX[1]);
			if (num < 20f)
			{
				HUDManager.Instance.ShakeCamera(ScreenShakeType.VeryStrong);
				GameNetworkManager.Instance.localPlayerController.JumpToFearLevel(0.7f);
			}
			else if (num < 50f)
			{
				HUDManager.Instance.ShakeCamera(ScreenShakeType.Long);
				GameNetworkManager.Instance.localPlayerController.JumpToFearLevel(0.5f);
			}
		}
		else
		{
			creatureAnimator.SetTrigger("Scream");
			peckAudio.pitch = UnityEngine.Random.Range(0.9f, 1.1f);
			peckAudio.PlayOneShot(squawkSFX);
			WalkieTalkie.TransmitOneShotAudio(peckAudio, squawkSFX);
			if (num < 16f)
			{
				GameNetworkManager.Instance.localPlayerController.JumpToFearLevel(0.2f);
				HUDManager.Instance.ShakeCamera(ScreenShakeType.Big);
			}
		}
		if (sync)
		{
			ScreechServerRpc(enraged, (int)GameNetworkManager.Instance.localPlayerController.playerClientId);
		}
	}

	[ServerRpc(RequireOwnership = false)]
	public void ScreechServerRpc(bool enraged, int playerWhoSent)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(2934149737u, serverRpcParams, RpcDelivery.Reliable);
				bufferWriter.WriteValueSafe(in enraged, default(FastBufferWriter.ForPrimitives));
				BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
				__endSendServerRpc(ref bufferWriter, 2934149737u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				ScreechClientRpc(enraged, playerWhoSent);
			}
		}
	}

	[ClientRpc]
	public void ScreechClientRpc(bool enraged, int playerWhoSent)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(3785748456u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in enraged, default(FastBufferWriter.ForPrimitives));
			BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
			__endSendClientRpc(ref bufferWriter, 3785748456u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (playerWhoSent != (int)GameNetworkManager.Instance.localPlayerController.playerClientId)
			{
				Screech(enraged);
			}
		}
	}

	private void CalculateLookingAnimation()
	{
		if (stunNormalizedTimer > 0f || (currentBehaviourStateIndex == 0 && idleBehaviour == 2) || (currentBehaviourStateIndex == 0 && idleBehaviour == 0 && miscTimer > 0.4f) || (isPeckingTree && currentBehaviourStateIndex == 0))
		{
			if (watchingThreat != null)
			{
				eyesLookTarget.position = watchingThreat.GetThreatLookTransform().position;
				eyesLookRigA.weight = Mathf.Lerp(eyesLookRigA.weight, 1f, Time.deltaTime * 16f);
				eyesLookRigB.weight = Mathf.Lerp(eyesLookRigB.weight, 1f, Time.deltaTime * 16f);
			}
			agent.angularSpeed = 520f;
			headLookRig.weight = Mathf.Lerp(headLookRig.weight, 0f, Time.deltaTime * 16f);
			return;
		}
		bool flag = currentBehaviourStateIndex == 1 && LostEggsFound.Count > 0 && carryingEgg && miscTimer > 0f;
		bool flag2 = creatureAnimator.GetBool("Attacking");
		bool flag3 = currentBehaviourStateIndex == 2 && Vector3.Distance(base.transform.position, destination) > 4f && Mathf.Abs(velX) > 0.3f;
		if (eyeTwitchInterval <= 0f)
		{
			eyeTwitchInterval = UnityEngine.Random.Range(0.1f, 0.6f);
			leftEyeMesh.localEulerAngles = baseEyeRotationLeft + new Vector3(UnityEngine.Random.Range(-1f, 1f) * eyeTwitchAmount, UnityEngine.Random.Range(-1f, 1f) * eyeTwitchAmount, UnityEngine.Random.Range(-1f, 1f) * eyeTwitchAmount);
			rightEyeMesh.localEulerAngles = baseEyeRotation + new Vector3(UnityEngine.Random.Range(-1f, 1f) * eyeTwitchAmount, UnityEngine.Random.Range(-1f, 1f) * eyeTwitchAmount, UnityEngine.Random.Range(-1f, 1f) * eyeTwitchAmount);
		}
		if (pingAttentionTimer >= 0f && !flag && !flag2)
		{
			lookTarget.position = Vector3.Lerp(lookTarget.position, pingAttentionPosition, 12f * Time.deltaTime);
		}
		else
		{
			if (watchingThreat == null || flag || flag2 || Physics.Linecast(eye.position, watchingThreat.GetThreatLookTransform().position, lookMask, QueryTriggerInteraction.Ignore))
			{
				agent.angularSpeed = 520f;
				headLookRig.weight = Mathf.Lerp(headLookRig.weight, 0f, Time.deltaTime * 16f);
				eyesLookRigA.weight = Mathf.Lerp(eyesLookRigA.weight, 0f, Time.deltaTime * 16f);
				eyesLookRigB.weight = Mathf.Lerp(eyesLookRigB.weight, 0f, Time.deltaTime * 16f);
				return;
			}
			lookTarget.position = Vector3.Lerp(lookTarget.position, watchingThreat.GetThreatLookTransform().position, 16f * Time.deltaTime);
		}
		if (base.IsOwner && !flag3 && Vector3.Angle(base.transform.forward, Vector3.Scale(new Vector3(1f, 0f, 1f), lookTarget.position - base.transform.position)) > 4f * Mathf.Min(Vector3.Distance(base.transform.position, lookTarget.position) * 0.5f, 10f))
		{
			agent.angularSpeed = 0f;
			turnCompass.LookAt(lookTarget);
			turnCompass.eulerAngles = new Vector3(0f, turnCompass.eulerAngles.y, 0f);
			float num = 7f;
			base.transform.rotation = Quaternion.Lerp(base.transform.rotation, turnCompass.rotation, num * Time.deltaTime);
			base.transform.localEulerAngles = new Vector3(0f, base.transform.localEulerAngles.y, 0f);
		}
		float num2 = ((!(Vector3.Dot(base.transform.right, Vector3.Normalize(Vector3.Scale(new Vector3(1f, 0f, 1f), lookTarget.position - base.transform.position))) > 0f)) ? 90f : (-90f));
		turnHeadInterval -= Time.deltaTime;
		if (turnHeadInterval < 0f)
		{
			turnHeadOffset = UnityEngine.Random.Range(-30f, 30f);
			turnHeadInterval = UnityEngine.Random.Range(0.6f, 1.6f);
		}
		headLookRig.data.offset = Vector3.Lerp(headLookRig.data.offset, new Vector3(0f, num2 + turnHeadOffset, 0f), Time.deltaTime * 50f);
		headLookTarget.position = Vector3.Lerp(headLookTarget.position, lookTarget.position, 8f * Time.deltaTime);
		eyesLookTarget.position = Vector3.Lerp(eyesLookTarget.position, lookTarget.position, 15f * Time.deltaTime);
		float num3 = Vector3.Angle(base.transform.forward, lookTarget.position - base.transform.position);
		if (num3 > 22f)
		{
			headLookRig.weight = Mathf.Lerp(headLookRig.weight, 1f * (Mathf.Abs(num3 - 180f) / 180f), Time.deltaTime * 11f);
		}
		else
		{
			headLookRig.weight = Mathf.Lerp(headLookRig.weight, 1f, Time.deltaTime * 11f);
		}
		eyesLookRigA.weight = Mathf.Lerp(eyesLookRigA.weight, 1f, Time.deltaTime * 16f);
		eyesLookRigB.weight = Mathf.Lerp(eyesLookRigB.weight, 1f, Time.deltaTime * 16f);
	}

	public override void AnimationEventA()
	{
		base.AnimationEventA();
		if (!inMovement)
		{
			return;
		}
		int num = UnityEngine.Random.Range(0, footstepSFX.Length);
		if (!(footstepSFX[num] == null))
		{
			creatureSFX.PlayOneShot(footstepSFX[num]);
			WalkieTalkie.TransmitOneShotAudio(creatureSFX, footstepSFX[num]);
			if (currentBehaviourStateIndex > 0 && Vector3.Distance(base.transform.position, destination) > 5f && !creatureAnimator.GetBool("Crawling"))
			{
				runningParticle.Play(withChildren: true);
			}
			if (currentBehaviourStateIndex > 0 && creatureAnimator.GetFloat("WalkSpeed") > 1f)
			{
				longDistanceAudio.PlayOneShot(footstepBassSFX[num]);
				WalkieTalkie.TransmitOneShotAudio(longDistanceAudio, footstepBassSFX[num]);
			}
			RoundManager.Instance.PlayAudibleNoise(base.transform.position, 12f, 0.6f, 0, isInsidePlayerShip && StartOfRound.Instance.hangarDoorsClosed, 675189);
		}
	}

	public override void AnimationEventB()
	{
		base.AnimationEventA();
		peckAudio.pitch = UnityEngine.Random.Range(0.95f, 1.05f);
		RoundManager.Instance.PlayAudibleNoise(eye.position, 25f, 0.85f, 0, isInsidePlayerShip && StartOfRound.Instance.hangarDoorsClosed, 675188);
		if (attacking && !isEnemyDead)
		{
			rocksParticle.Play();
			RoundManager.PlayRandomClip(peckAudio, attackSFX, randomize: true, 1f, 91911);
			timeSinceHittingGround = 0f;
			float num = Vector3.Distance(StartOfRound.Instance.audioListener.transform.position, base.transform.position);
			if (num < 5f)
			{
				HUDManager.Instance.ShakeCamera(ScreenShakeType.Big);
			}
			else if (num < 13f)
			{
				HUDManager.Instance.ShakeCamera(ScreenShakeType.Small);
			}
			destroyTreesInterval = (destroyTreesInterval + 1) % 4;
			if (destroyTreesInterval == 0)
			{
				RoundManager.Instance.DestroyTreeAtPosition(base.transform.position + base.transform.forward * 0.75f, 1.5f);
			}
			if (timeSinceHittingPlayer < 0.1f)
			{
				PlayerControllerB localPlayerController = GameNetworkManager.Instance.localPlayerController;
				_ = localPlayerController.transform.position;
				Vector3 vector = localPlayerController.transform.position + Vector3.up * 3f - base.transform.position;
				localPlayerController.externalForceAutoFade += vector * hitVelocityForce;
				localPlayerController.DamagePlayer(10, hasDamageSFX: true, callRPC: true, CauseOfDeath.Stabbing, 9, fallDamage: false, vector * hitVelocityForce * 0.4f);
				timeSinceHittingPlayer = 0f;
			}
		}
		else
		{
			woodChipParticle.Play();
			peckAudio.PlayOneShot(peckTreeSFX);
			WalkieTalkie.TransmitOneShotAudio(peckAudio, peckTreeSFX);
		}
	}

	public override void AnimationEventC()
	{
		base.AnimationEventC();
		if (isEnemyDead)
		{
			UnityEngine.Object.Destroy(base.gameObject);
		}
	}

	public override void AnimationEventD()
	{
		base.AnimationEventD();
		if (base.IsOwner && UnityEngine.Random.Range(0, 100) < 50)
		{
			Screech(enraged: false, sync: true);
		}
	}

	public override void OnDestroy()
	{
		base.OnDestroy();
		if (isEnemyDead)
		{
			UnityEngine.Object.Instantiate(feathersPrefab, base.transform.position, base.transform.rotation, RoundManager.Instance.mapPropsContainer.transform);
		}
	}

	private void CalculateAnimationDirection(float maxSpeed = 1f)
	{
		inMovement = velX > 0.2f || velZ > 0.2f;
		agentLocalVelocity = animationContainer.InverseTransformDirection(Vector3.ClampMagnitude(base.transform.position - previousPosition, 1f) / (Time.deltaTime * 2f));
		velX = Mathf.Lerp(velX, agentLocalVelocity.x, 10f * Time.deltaTime);
		creatureAnimator.SetFloat("VelocityX", Mathf.Clamp(velX, 0f - maxSpeed, maxSpeed));
		velZ = Mathf.Lerp(velZ, agentLocalVelocity.z, 10f * Time.deltaTime);
		creatureAnimator.SetFloat("VelocityZ", Mathf.Clamp(velZ, 0f - maxSpeed, maxSpeed));
		previousPosition = base.transform.position;
	}

	public override void DetectNoise(Vector3 noisePosition, float noiseLoudness, int timesPlayedInOneSpot = 0, int noiseID = 0)
	{
		base.DetectNoise(noisePosition, noiseLoudness, timesPlayedInOneSpot, noiseID);
		if (!base.IsOwner || (!base.IsOwner && noiseID != 75) || isEnemyDead || Time.realtimeSinceStartup - timeAtLastHeardNoise < 3f || Vector3.Distance(noisePosition, base.transform.position + Vector3.up * 0.4f) < 0.5f || (watchingThreat != null && timeSinceSeeingThreat < 1f && Vector3.Distance(noisePosition + Vector3.up * 0.4f, watchingThreat.GetThreatTransform().position) < 1f) || (currentBehaviourStateIndex == 2 && Vector3.Angle(base.transform.forward, noisePosition - base.transform.position) < 60f))
		{
			return;
		}
		float num = Vector3.Distance(noisePosition, base.transform.position);
		float num2 = noiseLoudness / num;
		if (Physics.Linecast(base.transform.position, noisePosition, StartOfRound.Instance.collidersAndRoomMaskAndDefault, QueryTriggerInteraction.Ignore))
		{
			num2 *= 0.6f;
		}
		if (pingAttentionTimer > 0f)
		{
			if (focusLevel >= 3)
			{
				if (num > 3f || num2 <= 0.12f)
				{
					return;
				}
			}
			else if (focusLevel == 2)
			{
				if (num > 25f || num2 <= 0.075f)
				{
					return;
				}
			}
			else if (focusLevel <= 1 && (num > 40f || num2 <= 0.06f))
			{
				return;
			}
		}
		if (!(num2 <= 0.022f))
		{
			timeAtLastHeardNoise = Time.realtimeSinceStartup;
			PingAttention(2, 0.5f, noisePosition + Vector3.up * 0.6f);
		}
	}

	public void PingAttention(int newFocusLevel, float timeToLook, Vector3 attentionPosition, bool sync = true)
	{
		if ((!(pingAttentionTimer >= 0f) || newFocusLevel >= focusLevel) && (currentBehaviourStateIndex != 0 || !(timeSincePingingAttention < 0.7f)) && (currentBehaviourStateIndex != 1 || !(timeSincePingingAttention < 0.4f)) && (currentBehaviourStateIndex != 2 || !(timeSincePingingAttention < 1f)))
		{
			focusLevel = newFocusLevel;
			pingAttentionTimer = timeToLook;
			pingAttentionPosition = attentionPosition;
			if (base.IsOwner && currentBehaviourStateIndex == 0 && idleBehaviour == 2)
			{
				idleBehaviour = 0;
				ChangeIdleBehaviorClientRpc(0);
			}
			if (sync)
			{
				PingBirdAttentionServerRpc(timeToLook, attentionPosition, (int)GameNetworkManager.Instance.localPlayerController.playerClientId);
			}
		}
	}

	[ServerRpc(RequireOwnership = false)]
	public void PingBirdAttentionServerRpc(float timeToLook, Vector3 attentionPosition, int playerWhoSent)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
			{
				ServerRpcParams serverRpcParams = default(ServerRpcParams);
				FastBufferWriter bufferWriter = __beginSendServerRpc(2000979301u, serverRpcParams, RpcDelivery.Reliable);
				bufferWriter.WriteValueSafe(in timeToLook, default(FastBufferWriter.ForPrimitives));
				bufferWriter.WriteValueSafe(in attentionPosition);
				BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
				__endSendServerRpc(ref bufferWriter, 2000979301u, serverRpcParams, RpcDelivery.Reliable);
			}
			if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
			{
				__rpc_exec_stage = __RpcExecStage.Send;
				PingBirdAttentionClientRpc(timeToLook, attentionPosition, playerWhoSent);
			}
		}
	}

	[ClientRpc]
	public void PingBirdAttentionClientRpc(float timeToLook, Vector3 attentionPosition, int playerWhoSent)
	{
		NetworkManager networkManager = base.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (__rpc_exec_stage != __RpcExecStage.Execute && (networkManager.IsServer || networkManager.IsHost))
		{
			ClientRpcParams clientRpcParams = default(ClientRpcParams);
			FastBufferWriter bufferWriter = __beginSendClientRpc(3294500800u, clientRpcParams, RpcDelivery.Reliable);
			bufferWriter.WriteValueSafe(in timeToLook, default(FastBufferWriter.ForPrimitives));
			bufferWriter.WriteValueSafe(in attentionPosition);
			BytePacker.WriteValueBitPacked(bufferWriter, playerWhoSent);
			__endSendClientRpc(ref bufferWriter, 3294500800u, clientRpcParams, RpcDelivery.Reliable);
		}
		if (__rpc_exec_stage == __RpcExecStage.Execute && (networkManager.IsClient || networkManager.IsHost))
		{
			__rpc_exec_stage = __RpcExecStage.Send;
			if (playerWhoSent != (int)GameNetworkManager.Instance.localPlayerController.playerClientId)
			{
				pingAttentionTimer = timeToLook;
				pingAttentionPosition = attentionPosition;
			}
		}
	}

	public override void OnCollideWithPlayer(Collider other)
	{
		base.OnCollideWithPlayer(other);
		PlayerControllerB playerControllerB = MeetsStandardPlayerCollisionConditions(other, inKillAnimation);
		if (playerControllerB != null && (!playerControllerB.isInHangarShipRoom || isInsidePlayerShip || !(playerControllerB.transform.position.y - base.transform.position.y > 1.6f) || !Physics.Linecast(base.transform.position + Vector3.up * 0.45f, playerControllerB.transform.position + Vector3.up * 0.45f, StartOfRound.Instance.collidersAndRoomMaskAndDefault, QueryTriggerInteraction.Ignore)))
		{
			timeSinceHittingPlayer = 0f;
			if (attacking)
			{
				GameNetworkManager.Instance.localPlayerController.JumpToFearLevel(1f);
			}
		}
	}

	public override void OnCollideWithEnemy(Collider other, EnemyAI collidedEnemy = null)
	{
		base.OnCollideWithEnemy(other, collidedEnemy);
		if (!collidedEnemy.isEnemyDead && attacking && timeSinceHittingGround < 0.2f)
		{
			if (collidedEnemy.enemyType.EnemySize == EnemySize.Tiny)
			{
				collidedEnemy.KillEnemy();
			}
			else if (Time.realtimeSinceStartup - timeSinceHittingEnemy > 0.15f)
			{
				timeSinceHittingEnemy = Time.realtimeSinceStartup;
				collidedEnemy.HitEnemy(4);
			}
		}
	}

	public override void KillEnemy(bool destroy = false)
	{
		if (creatureVoice != null)
		{
			creatureVoice.Stop();
		}
		creatureSFX.Stop();
		if (base.IsOwner)
		{
			FinishPryOpenDoor(cancelledEarly: true);
		}
		base.KillEnemy();
	}

	public override void HitEnemy(int force = 1, PlayerControllerB playerWhoHit = null, bool playHitSFX = false, int hitID = -1)
	{
		base.HitEnemy(force, playerWhoHit, playHitSFX, hitID);
		if (isEnemyDead)
		{
			return;
		}
		enemyHP -= force;
		timeSinceGettingHit = 0f;
		if (base.IsOwner && enemyHP <= 0)
		{
			KillEnemyOnOwnerClient();
		}
		if (playerWhoHit != null)
		{
			lastPlayerWhoAttacked = playerWhoHit;
			if (playerWhoHit.TryGetComponent<IVisibleThreat>(out var component))
			{
				ReactToThreatAttack(component);
			}
			return;
		}
		lastPlayerWhoAttacked = null;
		int num = Physics.OverlapSphereNonAlloc(eye.position, 6f, RoundManager.Instance.tempColliderResults, 524288, QueryTriggerInteraction.Collide);
		float num2 = 100f;
		int num3 = -1;
		for (int i = 0; i < num; i++)
		{
			float num4 = Vector3.Distance(eye.position, RoundManager.Instance.tempColliderResults[i].transform.position);
			if (num4 < num2)
			{
				num2 = num4;
				num3 = i;
			}
		}
		EnemyAICollisionDetect component2 = RoundManager.Instance.tempColliderResults[num3].transform.GetComponent<EnemyAICollisionDetect>();
		if (component2 != null && !component2.mainScript.isEnemyDead && component2.mainScript.TryGetComponent<IVisibleThreat>(out var component3))
		{
			ReactToThreatAttack(component3, doLOSCheck: true);
		}
	}

	protected override void __initializeVariables()
	{
		base.__initializeVariables();
	}

	protected override void __initializeRpcs()
	{
		__registerRpc(1509496551u, __rpc_handler_1509496551, "PeckTreeServerRpc");
		__registerRpc(2329433375u, __rpc_handler_2329433375, "PeckTreeClientRpc");
		__registerRpc(538806819u, __rpc_handler_538806819, "SetSleepingServerRpc");
		__registerRpc(1965629606u, __rpc_handler_1965629606, "SetSleepingClientRpc");
		__registerRpc(2757245750u, __rpc_handler_2757245750, "AddToThreatsHoldingEggListServerRpc");
		__registerRpc(321322077u, __rpc_handler_321322077, "AddToThreatsHoldingEggListClientRpc");
		__registerRpc(2000046764u, __rpc_handler_2000046764, "SyncWatchingThreatServerRpc");
		__registerRpc(4160656602u, __rpc_handler_4160656602, "SyncWatchingThreatClientRpc");
		__registerRpc(2370506911u, __rpc_handler_2370506911, "GrabScrapServerRpc");
		__registerRpc(3612179633u, __rpc_handler_3612179633, "GrabScrapClientRpc");
		__registerRpc(3671496547u, __rpc_handler_3671496547, "DropScrapServerRpc");
		__registerRpc(1775051963u, __rpc_handler_1775051963, "DropScrapClientRpc");
		__registerRpc(4019607224u, __rpc_handler_4019607224, "PryOpenDoorServerRpc");
		__registerRpc(2152766841u, __rpc_handler_2152766841, "PryOpenDoorClientRpc");
		__registerRpc(1803512774u, __rpc_handler_1803512774, "SetDancingServerRpc");
		__registerRpc(712000225u, __rpc_handler_712000225, "SetDancingClientRpc");
		__registerRpc(3238921108u, __rpc_handler_3238921108, "SpawnExplosionAtPlayerBodyServerRpc");
		__registerRpc(1143323347u, __rpc_handler_1143323347, "SpawnExplosionAtPlayerBodyClientRpc");
		__registerRpc(4085505192u, __rpc_handler_4085505192, "SyncWalkAnimSpeedServerRpc");
		__registerRpc(2658683365u, __rpc_handler_2658683365, "SyncWalkAnimSpeedClientRpc");
		__registerRpc(639546879u, __rpc_handler_639546879, "StartAttackingThreatServerRpc");
		__registerRpc(2397901577u, __rpc_handler_2397901577, "StartAttackingClientRpc");
		__registerRpc(3575423690u, __rpc_handler_3575423690, "ChangeIdleBehaviorClientRpc");
		__registerRpc(276418090u, __rpc_handler_276418090, "SpawnEggsClientRpc");
		__registerRpc(2934149737u, __rpc_handler_2934149737, "ScreechServerRpc");
		__registerRpc(3785748456u, __rpc_handler_3785748456, "ScreechClientRpc");
		__registerRpc(2000979301u, __rpc_handler_2000979301, "PingBirdAttentionServerRpc");
		__registerRpc(3294500800u, __rpc_handler_3294500800, "PingBirdAttentionClientRpc");
		base.__initializeRpcs();
	}

	private static void __rpc_handler_1509496551(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out bool value, default(FastBufferWriter.ForPrimitives));
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).PeckTreeServerRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2329433375(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out bool value, default(FastBufferWriter.ForPrimitives));
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).PeckTreeClientRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_538806819(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SetSleepingServerRpc();
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_1965629606(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SetSleepingClientRpc();
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2757245750(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			ByteUnpacker.ReadValueBitPacked(reader, out int value);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).AddToThreatsHoldingEggListServerRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_321322077(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			ByteUnpacker.ReadValueBitPacked(reader, out int value);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).AddToThreatsHoldingEggListClientRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2000046764(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out NetworkObjectReference value, default(FastBufferWriter.ForNetworkSerializable));
			ByteUnpacker.ReadValueBitPacked(reader, out int value2);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SyncWatchingThreatServerRpc(value, value2);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_4160656602(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out NetworkObjectReference value, default(FastBufferWriter.ForNetworkSerializable));
			ByteUnpacker.ReadValueBitPacked(reader, out int value2);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SyncWatchingThreatClientRpc(value, value2);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2370506911(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager == null || !networkManager.IsListening)
		{
			return;
		}
		if (rpcParams.Server.Receive.SenderClientId != target.OwnerClientId)
		{
			if (networkManager.LogLevel <= LogLevel.Normal)
			{
				Debug.LogError("Only the owner can invoke a ServerRpc that requires ownership!");
			}
		}
		else
		{
			reader.ReadValueSafe(out NetworkObjectReference value, default(FastBufferWriter.ForNetworkSerializable));
			ByteUnpacker.ReadValueBitPacked(reader, out int value2);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).GrabScrapServerRpc(value, value2);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_3612179633(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out NetworkObjectReference value, default(FastBufferWriter.ForNetworkSerializable));
			ByteUnpacker.ReadValueBitPacked(reader, out int value2);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).GrabScrapClientRpc(value, value2);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_3671496547(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out NetworkObjectReference value, default(FastBufferWriter.ForNetworkSerializable));
			reader.ReadValueSafe(out Vector3 value2);
			reader.ReadValueSafe(out bool value3, default(FastBufferWriter.ForPrimitives));
			ByteUnpacker.ReadValueBitPacked(reader, out int value4);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).DropScrapServerRpc(value, value2, value3, value4);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_1775051963(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out NetworkObjectReference value, default(FastBufferWriter.ForNetworkSerializable));
			reader.ReadValueSafe(out Vector3 value2);
			reader.ReadValueSafe(out bool value3, default(FastBufferWriter.ForPrimitives));
			ByteUnpacker.ReadValueBitPacked(reader, out int value4);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).DropScrapClientRpc(value, value2, value3, value4);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_4019607224(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			ByteUnpacker.ReadValueBitPacked(reader, out int value);
			reader.ReadValueSafe(out bool value2, default(FastBufferWriter.ForPrimitives));
			reader.ReadValueSafe(out bool value3, default(FastBufferWriter.ForPrimitives));
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).PryOpenDoorServerRpc(value, value2, value3);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2152766841(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			ByteUnpacker.ReadValueBitPacked(reader, out int value);
			reader.ReadValueSafe(out bool value2, default(FastBufferWriter.ForPrimitives));
			reader.ReadValueSafe(out bool value3, default(FastBufferWriter.ForPrimitives));
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).PryOpenDoorClientRpc(value, value2, value3);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_1803512774(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out bool value, default(FastBufferWriter.ForPrimitives));
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SetDancingServerRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_712000225(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out bool value, default(FastBufferWriter.ForPrimitives));
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SetDancingClientRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_3238921108(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out Vector3 value);
			reader.ReadValueSafe(out Quaternion value2);
			ByteUnpacker.ReadValueBitPacked(reader, out int value3);
			ByteUnpacker.ReadValueBitPacked(reader, out int value4);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SpawnExplosionAtPlayerBodyServerRpc(value, value2, value3, value4);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_1143323347(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out Vector3 value);
			reader.ReadValueSafe(out Quaternion value2);
			ByteUnpacker.ReadValueBitPacked(reader, out int value3);
			ByteUnpacker.ReadValueBitPacked(reader, out int value4);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SpawnExplosionAtPlayerBodyClientRpc(value, value2, value3, value4);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_4085505192(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out float value, default(FastBufferWriter.ForPrimitives));
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SyncWalkAnimSpeedServerRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2658683365(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out float value, default(FastBufferWriter.ForPrimitives));
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SyncWalkAnimSpeedClientRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_639546879(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out NetworkObjectReference value, default(FastBufferWriter.ForNetworkSerializable));
			ByteUnpacker.ReadValueBitPacked(reader, out int value2);
			reader.ReadValueSafe(out bool value3, default(FastBufferWriter.ForPrimitives));
			int[] value4 = null;
			if (value3)
			{
				reader.ReadValueSafe(out value4, default(FastBufferWriter.ForPrimitives));
			}
			ByteUnpacker.ReadValueBitPacked(reader, out int value5);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).StartAttackingThreatServerRpc(value, value2, value4, value5);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2397901577(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out NetworkObjectReference value, default(FastBufferWriter.ForNetworkSerializable));
			ByteUnpacker.ReadValueBitPacked(reader, out int value2);
			ByteUnpacker.ReadValueBitPacked(reader, out int value3);
			reader.ReadValueSafe(out bool value4, default(FastBufferWriter.ForPrimitives));
			int[] value5 = null;
			if (value4)
			{
				reader.ReadValueSafe(out value5, default(FastBufferWriter.ForPrimitives));
			}
			ByteUnpacker.ReadValueBitPacked(reader, out int value6);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).StartAttackingClientRpc(value, value2, value3, value5, value6);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_3575423690(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			ByteUnpacker.ReadValueBitPacked(reader, out int value);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).ChangeIdleBehaviorClientRpc(value);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_276418090(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out bool value, default(FastBufferWriter.ForPrimitives));
			NetworkObjectReference[] value2 = null;
			if (value)
			{
				reader.ReadValueSafe(out value2, default(FastBufferWriter.ForNetworkSerializable));
			}
			reader.ReadValueSafe(out bool value3, default(FastBufferWriter.ForPrimitives));
			int[] value4 = null;
			if (value3)
			{
				reader.ReadValueSafe(out value4, default(FastBufferWriter.ForPrimitives));
			}
			reader.ReadValueSafe(out bool value5, default(FastBufferWriter.ForPrimitives));
			Vector3[] value6 = null;
			if (value5)
			{
				reader.ReadValueSafe(out value6);
			}
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).SpawnEggsClientRpc(value2, value4, value6);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2934149737(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out bool value, default(FastBufferWriter.ForPrimitives));
			ByteUnpacker.ReadValueBitPacked(reader, out int value2);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).ScreechServerRpc(value, value2);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_3785748456(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out bool value, default(FastBufferWriter.ForPrimitives));
			ByteUnpacker.ReadValueBitPacked(reader, out int value2);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).ScreechClientRpc(value, value2);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_2000979301(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out float value, default(FastBufferWriter.ForPrimitives));
			reader.ReadValueSafe(out Vector3 value2);
			ByteUnpacker.ReadValueBitPacked(reader, out int value3);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).PingBirdAttentionServerRpc(value, value2, value3);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	private static void __rpc_handler_3294500800(NetworkBehaviour target, FastBufferReader reader, __RpcParams rpcParams)
	{
		NetworkManager networkManager = target.NetworkManager;
		if ((object)networkManager != null && networkManager.IsListening)
		{
			reader.ReadValueSafe(out float value, default(FastBufferWriter.ForPrimitives));
			reader.ReadValueSafe(out Vector3 value2);
			ByteUnpacker.ReadValueBitPacked(reader, out int value3);
			target.__rpc_exec_stage = __RpcExecStage.Execute;
			((GiantKiwiAI)target).PingBirdAttentionClientRpc(value, value2, value3);
			target.__rpc_exec_stage = __RpcExecStage.Send;
		}
	}

	protected internal override string __getTypeName()
	{
		return "GiantKiwiAI";
	}
}
